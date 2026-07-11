use axum::extract::State;
use axum::{Extension, Json, http::StatusCode, middleware};
use chrono::{DateTime, NaiveDate, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::auth::{AuthState, AuthUser};
use crate::middleware::require_auth;
use crate::core_types::Task;

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
    "Responde ÚNICAMENTE con la sentencia SQL, sin explicaciones ni formato markdown.",
);

const NL_PROMPT_PREFIX: &str = "Eres un asistente que responde preguntas sobre tareas en español.\n\nLa pregunta del usuario era: \"";

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

#[allow(clippy::too_many_lines)]
fn execute_sql(sql: &str, tasks: &[Task]) -> Vec<Task> {
    let sql_lower = sql.trim().to_lowercase();
    if !sql_lower.starts_with("select") {
        return Vec::new();
    }

    let where_clause = sql_lower
        .split("order by")
        .next()
        .unwrap_or("")
        .split("limit")
        .next()
        .unwrap_or("")
        .split("select * from tasks")
        .last()
        .unwrap_or("")
        .trim();
    let where_str = where_clause
        .strip_prefix("where")
        .unwrap_or(where_clause)
        .trim();
    let where_str = where_str
        .strip_prefix('(')
        .unwrap_or(where_str)
        .strip_suffix(')')
        .unwrap_or(where_str);

    let order_clause = sql_lower
        .split("order by")
        .nth(1)
        .and_then(|s| s.split("limit").next())
        .map_or("", str::trim);
    let limit_clause = sql_lower
        .split("limit")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<usize>().ok());

    let mut filtered: Vec<&Task> = tasks.iter().collect();

    if !where_str.is_empty() {
        let conditions = split_conditions(where_str);
        filtered.retain(|task| conditions.iter().all(|cond| evaluate_condition(task, cond)));
    }

    if !order_clause.is_empty() {
        let parts: Vec<&str> = order_clause.split_whitespace().collect();
        let col = parts.first().copied().unwrap_or("");
        let desc = parts.get(1).copied().unwrap_or("asc") == "desc";
        sort_tasks(&mut filtered, col, desc);
    }

    if let Some(limit) = limit_clause {
        filtered.truncate(limit);
    }

    filtered.into_iter().cloned().collect()
}

fn split_conditions(where_str: &str) -> Vec<String> {
    let mut conditions = Vec::new();
    let mut depth = 0;
    let mut current = String::new();

    for ch in where_str.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            'a' | 'n' | 'd' if depth == 0 && current.trim().ends_with(" an") && ch == 'd' => {
                let trimmed = current.trim_end();
                if let Some(stripped) = trimmed.strip_suffix(" an") {
                    let cond = stripped.trim().to_string();
                    if !cond.is_empty() {
                        conditions.push(cond);
                    }
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
            'o' | 'r' if depth == 0 && current.trim().ends_with(" o") && ch == 'r' => {
                let trimmed = current.trim_end();
                if let Some(stripped) = trimmed.strip_suffix(" o") {
                    let cond = stripped.trim().to_string();
                    if !cond.is_empty() {
                        conditions.push(cond);
                    }
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    let remaining = current.trim().to_string();
    if !remaining.is_empty() && remaining != "and" && remaining != "or" {
        conditions.push(remaining);
    }

    conditions
}

fn evaluate_condition(task: &Task, condition: &str) -> bool {
    let condition = condition.trim();

    if let Some(not_cond) = condition.strip_prefix("not ") {
        return !evaluate_condition(task, not_cond.trim());
    }

    let re = Regex::new(r"^(\w+)\s*(=|!=|>=|<=|>|<|like|ilike|in)\s*(.+)$").unwrap();
    let Some(caps) = re.captures(condition) else {
        return true;
    };

    let column = caps.get(1).unwrap().as_str();
    let op = caps.get(2).unwrap().as_str();
    let raw_value = caps.get(3).unwrap().as_str().trim().trim_matches('\'');

    let task_val = get_column_value(task, column);
    let cond_val = raw_value.to_lowercase();

    match (op.to_lowercase().as_str(), &task_val) {
        ("=", ColumnValue::String(s)) => s.to_lowercase() == cond_val,
        ("=", ColumnValue::Bool(b)) => {
            let wanted = cond_val == "true" || cond_val == "1";
            *b == wanted
        }
        ("=", ColumnValue::Char(c)) => c.is_some_and(|c| c.to_lowercase().to_string() == cond_val),
        ("=", ColumnValue::Date(d)) => d.is_some_and(|d| d.to_string() == cond_val),
        ("=", ColumnValue::DateTime(d)) => {
            d.is_some_and(|d| d.format("%Y-%m-%d").to_string() == cond_val)
        }
        ("=", ColumnValue::StringArray(arr)) => arr.contains(&cond_val),

        ("!=", ColumnValue::String(s)) => s.to_lowercase() != cond_val,
        ("!=", ColumnValue::Bool(b)) => {
            let wanted = cond_val == "true" || cond_val == "1";
            *b != wanted
        }
        ("!=", ColumnValue::Char(c)) => c.is_none_or(|c| c.to_lowercase().to_string() != cond_val),
        ("!=", ColumnValue::Date(d)) => d.is_none_or(|d| d.to_string() != cond_val),

        (">", ColumnValue::Date(d)) => {
            let parsed = NaiveDate::parse_from_str(&cond_val, "%Y-%m-%d").ok();
            d.is_some() && parsed.is_some() && d.unwrap() > parsed.unwrap()
        }
        (">", ColumnValue::DateTime(d)) => {
            let parsed = NaiveDate::parse_from_str(&cond_val, "%Y-%m-%d")
                .ok()
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
            d.is_some() && parsed.is_some() && d.unwrap() > parsed.unwrap()
        }
        ("<", ColumnValue::Date(d)) => {
            let parsed = NaiveDate::parse_from_str(&cond_val, "%Y-%m-%d").ok();
            d.is_some() && parsed.is_some() && d.unwrap() < parsed.unwrap()
        }
        ("<", ColumnValue::DateTime(d)) => {
            let parsed = NaiveDate::parse_from_str(&cond_val, "%Y-%m-%d")
                .ok()
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
            d.is_some() && parsed.is_some() && d.unwrap() < parsed.unwrap()
        }
        (">=", ColumnValue::Date(d)) => {
            let parsed = NaiveDate::parse_from_str(&cond_val, "%Y-%m-%d").ok();
            d.is_some() && parsed.is_some() && d.unwrap() >= parsed.unwrap()
        }
        ("<=", ColumnValue::Date(d)) => {
            let parsed = NaiveDate::parse_from_str(&cond_val, "%Y-%m-%d").ok();
            d.is_some() && parsed.is_some() && d.unwrap() <= parsed.unwrap()
        }

        ("like" | "ilike", ColumnValue::String(s)) => {
            let pattern = cond_val.replace('%', ".*").replace('_', ".");
            let re = Regex::new(&format!("(?i)^{pattern}$")).unwrap();
            re.is_match(s)
        }

        _ => true,
    }
}

enum ColumnValue {
    String(String),
    Bool(bool),
    Char(Option<char>),
    Date(Option<NaiveDate>),
    DateTime(Option<DateTime<Utc>>),
    StringArray(Vec<String>),
}

fn get_column_value(task: &Task, column: &str) -> ColumnValue {
    match column {
        "description" => ColumnValue::String(task.description.clone()),
        "status" => ColumnValue::String(format!("{:?}", task.status).to_lowercase()),
        "completed" => ColumnValue::Bool(task.completed),
        "priority" => ColumnValue::Char(task.priority),
        "created_at" => ColumnValue::DateTime(Some(task.created_at)),
        "updated_at" => ColumnValue::DateTime(Some(task.updated_at)),
        "completed_at" => ColumnValue::DateTime(task.completed_at),
        "due_date" => ColumnValue::Date(task.due_date),
        "project_ids" => {
            ColumnValue::StringArray(task.project_ids.iter().map(ToString::to_string).collect())
        }
        "context_ids" => {
            ColumnValue::StringArray(task.context_ids.iter().map(ToString::to_string).collect())
        }
        _ => ColumnValue::String(String::new()),
    }
}

fn sort_tasks(tasks: &mut Vec<&Task>, column: &str, desc: bool) {
    let cmp = |a: &Task, b: &Task| -> std::cmp::Ordering {
        match column {
            "created_at" => a.created_at.cmp(&b.created_at),
            "updated_at" => a.updated_at.cmp(&b.updated_at),
            "due_date" => a.due_date.cmp(&b.due_date),
            "priority" => a.priority.cmp(&b.priority),
            "description" => a.description.cmp(&b.description),
            "status" => format!("{:?}", a.status).cmp(&format!("{:?}", b.status)),
            _ => std::cmp::Ordering::Equal,
        }
    };
    if desc {
        tasks.sort_by(|a, b| cmp(b, a));
    } else {
        tasks.sort_by(|a, b| cmp(a, b));
    }
}

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

    let sql = extract_sql(&sql_response);
    tracing::info!(sql = %sql, "generated SQL");

    // Step 2: Fetch all tasks from ParadeDB and execute SQL in-memory
    let all_tasks = state.db.list("").await.map_err(|e| {
        tracing::error!(error = %e, "list tasks for query failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    let results = execute_sql(&sql, &all_tasks);
    tracing::info!(count = results.len(), "SQL executed");

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
            format!(
                "- [{status}]{prio} {}{due}",
                t.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn query_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route("/api/query", axum::routing::post(handle_query))
        .layer(middleware::from_fn_with_state(state, require_auth))
}