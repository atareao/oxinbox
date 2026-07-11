use axum::extract::State;
use axum::{Extension, Json, http::StatusCode, middleware};
use serde::{Deserialize, Serialize};
use sqlx::AssertSqlSafe;
use sqlx::Row;
use tracing::instrument;

use crate::auth::{AuthState, AuthUser};
use crate::core_types::{Task, TaskStatus, Uuid};
use crate::middleware::require_auth;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub sql: String,
    pub results: Vec<Task>,
    pub answer: String,
}

// ---------------------------------------------------------------------------
// Prompts
// ---------------------------------------------------------------------------

const SQL_GEN_PROMPT: &str = concat!(
    "Eres un asistente que convierte preguntas en lenguaje natural a SQL para PostgreSQL.\n\n",
    "Esquema de la base de datos:\n",
    "CREATE TABLE tasks (\n",
    "    id          UUID PRIMARY KEY,\n",
    "    completed   BOOLEAN,\n",
    "    priority    CHAR(1),\n",
    "    description TEXT,\n",
    "    project_ids UUID[],\n",
    "    context_ids UUID[],\n",
    "    status      TEXT CHECK (status IN ('inbox', 'todo', 'doing', 'done', 'someday')),\n",
    "    created_at  TIMESTAMPTZ,\n",
    "    updated_at  TIMESTAMPTZ,\n",
    "    completed_at TIMESTAMPTZ,\n",
    "    due_date    DATE\n",
    ");\n\n",
    "Reglas:\n",
    "- Usa `ILIKE` para búsquedas de texto (ej: description ILIKE '%term%')\n",
    "- project_ids y context_ids son UUID[], usa `ANY()`\n",
    "- Para buscar en arrays usa: `'uuid-aqui'::UUID = ANY(project_ids)`\n",
    "- status puede ser: 'inbox', 'todo', 'doing', 'done', 'someday'\n",
    "- dates están en formato ISO 8601 (YYYY-MM-DD)\n",
    "- priority es un CHAR(1): 'A', 'B', 'C', o NULL\n\n",
    "Siempre usa SELECT * y no añadas más columnas de las necesarias.\n\n",
    "Responde ÚNICAMENTE con la sentencia SQL, sin explicaciones ni formato markdown.",
);

const NL_PROMPT_PREFIX: &str = "Eres un asistente que responde preguntas sobre tareas en español.\n\nLa pregunta del usuario era: \"";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract SQL from LLM response, stripping markdown fences.
fn extract_sql(text: &str) -> String {
    let text = text.trim();
    if let Some(stripped) = text
        .strip_prefix("```sql")
        .or_else(|| text.strip_prefix("```"))
    {
        if let Some(end) = stripped.find("```") {
            return stripped[..end].trim().to_string();
        }
        return stripped.trim().to_string();
    }
    text.to_string()
}

/// Validate SQL: must be a SELECT on tasks, add LIMIT if missing.
fn prepare_sql(sql: &str) -> Result<String, String> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let upper = trimmed.to_uppercase();

    if !upper.starts_with("SELECT") {
        return Err("only SELECT queries are allowed".into());
    }

    // Add LIMIT 100 if not present
    if upper.contains("LIMIT") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed} LIMIT 100"))
    }
}

/// Parse a ParadeDB row into a Task, falling back to defaults for missing columns.
fn row_to_task(row: &sqlx::postgres::PgRow) -> Task {
    let status_str: String = row.try_get("status").unwrap_or_default();
    let status = match status_str.to_lowercase().as_str() {
        "inbox" => TaskStatus::Inbox,
        "todo" => TaskStatus::Todo,
        "doing" => TaskStatus::Doing,
        "done" => TaskStatus::Done,
        _ => TaskStatus::Someday,
    };

    Task {
        id: row.try_get("id").unwrap_or_else(|_| Uuid::now_v7()),
        completed: row.try_get("completed").unwrap_or(false),
        priority: row
            .try_get::<Option<String>, _>("priority")
            .ok()
            .flatten()
            .and_then(|p| p.chars().next())
            .filter(char::is_ascii_uppercase),
        description: row.try_get("description").unwrap_or_default(),
        project_ids: row.try_get("project_ids").unwrap_or_default(),
        context_ids: row.try_get("context_ids").unwrap_or_default(),
        status,
        created_at: row
            .try_get("created_at")
            .unwrap_or_else(|_| chrono::Utc::now()),
        updated_at: row
            .try_get("updated_at")
            .unwrap_or_else(|_| chrono::Utc::now()),
        completed_at: row.try_get("completed_at").ok().flatten(),
        due_date: row.try_get("due_date").ok().flatten(),
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

#[instrument(skip(state, _user), fields(query = %req.query))]
pub async fn handle_query(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, (StatusCode, String)> {
    let ai = state.ai_provider.as_ref().ok_or_else(|| {
        tracing::warn!("AI not configured for query endpoint");
        (StatusCode::SERVICE_UNAVAILABLE, "AI not configured".into())
    })?;

    // Step 1: Generate SQL from natural language
    let sql_response = ai
        .chat(
            SQL_GEN_PROMPT,
            &[crate::ai::ChatMessage {
                role: crate::ai::ChatRole::User,
                content: req.query.clone(),
            }],
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "SQL generation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("SQL generation failed: {e}"),
            )
        })?;

    let raw_sql = extract_sql(&sql_response);
    let sql = prepare_sql(&raw_sql).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid generated SQL: {e}: {raw_sql}"),
        )
    })?;
    tracing::info!(sql = %sql, "generated SQL");

    // Step 2: Execute SQL directly against ParadeDB
    // Safety: we validated the SQL is SELECT-only and added LIMIT above
    let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
        .fetch_all(state.db.pool())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, sql = %sql, "ParadeDB query failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("database query failed: {e}"),
            )
        })?;

    let results: Vec<Task> = rows.iter().map(row_to_task).collect();
    tracing::info!(count = results.len(), "ParadeDB query executed");

    // Step 3: Convert results to natural language
    let answer_prompt = format!(
        "{}{}\"\n\nLa consulta SQL ejecutada fue:\n```sql\n{}\n```\n\nResultados ({} filas):\n{}\n\nResponde en español de forma natural y concisa.",
        NL_PROMPT_PREFIX,
        req.query,
        sql,
        results.len(),
        format_results(&results),
    );

    let answer = ai
        .chat(
            "Eres un asistente útil que responde en español.",
            &[crate::ai::ChatMessage {
                role: crate::ai::ChatRole::User,
                content: answer_prompt,
            }],
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "answer generation failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("answer generation failed: {e}"),
            )
        })?;

    Ok(Json(QueryResponse {
        sql,
        results,
        answer,
    }))
}

fn format_results(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return "(sin resultados)".into();
    }
    tasks
        .iter()
        .map(|t| {
            let status = format!("{:?}", t.status).to_lowercase();
            let prio = t.priority.map_or(String::new(), |p| format!(" ({p})"));
            let due = t
                .due_date
                .map_or(String::new(), |d| format!(" [vence: {d}]"));
            format!("- [{status}]{prio} {}{due}", t.description)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Route factory
// ---------------------------------------------------------------------------

pub fn query_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route("/api/query", axum::routing::post(handle_query))
        .layer(middleware::from_fn_with_state(state, require_auth))
}
