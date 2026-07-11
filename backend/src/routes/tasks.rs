use axum::extract::State;
use axum::{Extension, Json, extract::Path, http::StatusCode, middleware};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::ai::task_builder;
use crate::auth::{AuthState, AuthUser};
use crate::core_types::{Context, Project, Task, TaskHistoryEntry};
use crate::database::ParadeDbRepository;
use crate::middleware::require_auth;
use crate::repository::RepositoryError;

// ---------------------------------------------------------------------------
// Route factory
// ---------------------------------------------------------------------------

pub fn task_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route(
            "/api/tasks",
            axum::routing::get(list_tasks).post(create_task),
        )
        .route(
            "/api/tasks/{id}",
            axum::routing::get(get_task)
                .put(update_task)
                .delete(delete_task),
        )
        .route("/api/tasks/search", axum::routing::post(search_tasks))
        .route(
            "/api/tasks/{id}/history",
            axum::routing::get(get_task_history),
        )
        .layer(middleware::from_fn_with_state(state, require_auth))
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub description: String,
    #[serde(default)]
    pub priority: Option<char>,
    #[serde(default)]
    pub project_ids: Vec<Uuid>,
    #[serde(default)]
    pub context_ids: Vec<Uuid>,
    #[serde(default)]
    pub due_date: Option<chrono::NaiveDate>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub task: Task,
    pub score: f64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn auto_assign_context(
    ai: &dyn crate::ai::AiProvider,
    description: &str,
    projects: &[Project],
    contexts: &[Context],
) -> Result<(Vec<Uuid>, Vec<Uuid>), String> {
    if projects.is_empty() && contexts.is_empty() {
        return Ok((vec![], vec![]));
    }

    let project_names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
    let context_names: Vec<&str> = contexts.iter().map(|c| c.name.as_str()).collect();

    let system_prompt = format!(
        concat!(
            "Eres un asistente que asigna proyectos y contextos a tareas.\n\n",
            "Proyectos disponibles: {}\n",
            "Contextos disponibles: {}\n\n",
            "Responde ÚNICAMENTE con un JSON: {{\"projects\":[],\"contexts\":[]}}\n",
            "Selecciona solo los que más se relacionen con la tarea. Si ninguno, listas vacías."
        ),
        serde_json::to_string(&project_names).unwrap_or_default(),
        serde_json::to_string(&context_names).unwrap_or_default()
    );

    let messages = vec![crate::ai::ChatMessage {
        role: crate::ai::ChatRole::User,
        content: description.to_string(),
    }];

    let result = ai
        .chat(&system_prompt, &messages)
        .await
        .map_err(|e| e.to_string())?;

    #[derive(serde::Deserialize)]
    struct AutoAssignResponse {
        projects: Vec<String>,
        contexts: Vec<String>,
    }

    let parsed: AutoAssignResponse = serde_json::from_str(&result)
        .map_err(|e| format!("AI response parse failed: {e}: {result}"))?;

    let project_ids: Vec<Uuid> = projects
        .iter()
        .filter(|p| parsed.projects.contains(&p.name))
        .map(|p| p.id)
        .collect();

    let context_ids: Vec<Uuid> = contexts
        .iter()
        .filter(|c| parsed.contexts.contains(&c.name))
        .map(|c| c.id)
        .collect();

    Ok((project_ids, context_ids))
}

async fn record_changes(db: &ParadeDbRepository, old: &Task, new: &Task, task_id: Uuid) {
    let now = chrono::Utc::now();

    let mut changes = Vec::new();

    if old.description != new.description {
        changes.push(TaskHistoryEntry {
            id: 0,
            task_id,
            field_name: "description".into(),
            old_value: Some(old.description.clone()),
            new_value: new.description.clone(),
            changed_at: now,
        });
    }
    if old.priority != new.priority {
        changes.push(TaskHistoryEntry {
            id: 0,
            task_id,
            field_name: "priority".into(),
            old_value: old.priority.map(|c| c.to_string()),
            new_value: new
                .priority
                .map_or_else(|| "null".into(), |c| c.to_string()),
            changed_at: now,
        });
    }
    if old.status != new.status {
        changes.push(TaskHistoryEntry {
            id: 0,
            task_id,
            field_name: "status".into(),
            old_value: Some(format!("{:?}", old.status).to_lowercase()),
            new_value: format!("{:?}", new.status).to_lowercase(),
            changed_at: now,
        });
    }
    if old.project_ids != new.project_ids {
        changes.push(TaskHistoryEntry {
            id: 0,
            task_id,
            field_name: "project_ids".into(),
            old_value: Some(serde_json::to_string(&old.project_ids).unwrap_or_default()),
            new_value: serde_json::to_string(&new.project_ids).unwrap_or_default(),
            changed_at: now,
        });
    }
    if old.context_ids != new.context_ids {
        changes.push(TaskHistoryEntry {
            id: 0,
            task_id,
            field_name: "context_ids".into(),
            old_value: Some(serde_json::to_string(&old.context_ids).unwrap_or_default()),
            new_value: serde_json::to_string(&new.context_ids).unwrap_or_default(),
            changed_at: now,
        });
    }
    if old.due_date != new.due_date {
        changes.push(TaskHistoryEntry {
            id: 0,
            task_id,
            field_name: "due_date".into(),
            old_value: Some(
                old.due_date
                    .map_or_else(|| "null".into(), |d| d.to_string()),
            ),
            new_value: new
                .due_date
                .map_or_else(|| "null".into(), |d| d.to_string()),
            changed_at: now,
        });
    }

    for entry in &changes {
        if let Err(e) = db.insert_field_change(entry).await {
            tracing::warn!(task_id = %task_id, error = %e, "failed to record field change");
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[instrument(skip(state), fields(description = %req.description))]
pub async fn create_task(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let now = chrono::Utc::now();
    let mut task = Task {
        id: Uuid::now_v7(),
        completed: false,
        priority: req.priority.filter(char::is_ascii_uppercase),
        description: req.description,
        project_ids: req.project_ids,
        context_ids: req.context_ids,
        status: crate::core_types::TaskStatus::Inbox,
        created_at: now,
        updated_at: now,
        completed_at: None,
        due_date: req.due_date,
    };

    // Auto-assign projects / contexts when none provided
    if task.project_ids.is_empty()
        && task.context_ids.is_empty()
        && let Some(ai) = &state.ai_provider
    {
        let projects = state.db.list_projects().await.unwrap_or_default();
        let contexts = state.db.list_contexts().await.unwrap_or_default();
        match auto_assign_context(ai.as_ref(), &task.description, &projects, &contexts).await {
            Ok((p_ids, c_ids)) => {
                if !p_ids.is_empty() || !c_ids.is_empty() {
                    tracing::info!(task_id = %task.id, project_ids = ?p_ids, context_ids = ?c_ids, "auto-assigned");
                    task.project_ids = p_ids;
                    task.context_ids = c_ids;
                }
            }
            Err(e) => {
                tracing::warn!(task_id = %task.id, error = %e, "auto-assign failed");
            }
        }
    }

    let created = state.db.create(&task, &user.user_id).await.map_err(|e| {
        tracing::error!(error = %e, "db create failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    if let Some(ai) = &state.ai_provider {
        task_builder::generate_and_store_embedding(ai.as_ref(), &task, &state.db).await;
    }

    tracing::info!(task_id = %created.id, "task created");
    Ok(Json(created))
}

#[instrument(skip(state))]
pub async fn list_tasks(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<Task>>, (StatusCode, String)> {
    let tasks = state.db.list(&user.user_id).await.map_err(|e| {
        tracing::error!(error = %e, "db list failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    tracing::info!(count = tasks.len(), "tasks listed");
    Ok(Json(tasks))
}

#[instrument(skip(state), fields(task_id = %id))]
pub async fn get_task(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let task = state.db.get(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!("task not found");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "get task database error");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;
    Ok(Json(task))
}

#[instrument(skip(state, _user), fields(task_id = %id))]
pub async fn update_task(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(updates): Json<CreateTaskRequest>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let mut task = state.db.get(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!("task not found for update");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "get task database error");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    let old = task.clone();

    task.description = updates.description;
    task.priority = updates.priority.filter(char::is_ascii_uppercase);
    task.project_ids = updates.project_ids;
    task.context_ids = updates.context_ids;
    task.due_date = updates.due_date;

    if let Some(status_str) = &updates.status {
        task.status = match status_str.to_lowercase().as_str() {
            "inbox" => crate::core_types::TaskStatus::Inbox,
            "todo" => crate::core_types::TaskStatus::Todo,
            "doing" => crate::core_types::TaskStatus::Doing,
            "done" => crate::core_types::TaskStatus::Done,
            "someday" => crate::core_types::TaskStatus::Someday,
            _ => task.status,
        };
    }

    record_changes(&state.db, &old, &task, id).await;

    let result = state.db.update(&task).await.map_err(|e| {
        tracing::error!(error = %e, "db update failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    if let Some(ai) = &state.ai_provider {
        task_builder::generate_and_store_embedding(ai.as_ref(), &task, &state.db).await;
    }

    tracing::info!("task updated");
    Ok(Json(result))
}

#[instrument(skip(state), fields(task_id = %id))]
pub async fn delete_task(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.db.delete(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!("task not found for deletion");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "delete task database error");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;
    tracing::info!("task deleted");
    Ok(StatusCode::NO_CONTENT)
}

#[instrument(skip(state), fields(query = %req.query))]
pub async fn search_tasks(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, (StatusCode, String)> {
    let limit = req.limit.unwrap_or(20).min(100);

    let query_embedding = if let Some(ai) = &state.ai_provider {
        match ai.embed(&req.query).await {
            Ok(emb) => Some(emb),
            Err(e) => {
                tracing::warn!(error = %e, "query embedding failed, falling back to BM25-only");
                None
            }
        }
    } else {
        None
    };

    let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);

    // BM25 search via ParadeDB
    let bm25_tasks = state
        .db
        .search_bm25(&user.user_id, &req.query, limit_i64)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "db search failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Vector search via ParadeDB (if we have an embedding)
    let vector_results = if let Some(emb) = query_embedding {
        state
            .db
            .search_vector(&user.user_id, &emb, limit_i64)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Merge BM25 + vector results, deduplicate by id
    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    for (task, bm25_score) in bm25_tasks {
        if seen.insert(task.id) {
            // BM25 scores are unbounded; normalize to [0, 1] range for RRF
            let score = (bm25_score / (1.0 + bm25_score)).min(1.0);
            results.push(SearchResult { task, score });
        }
    }
    for (task, vec_score) in vector_results {
        if seen.insert(task.id) {
            results.push(SearchResult {
                task,
                score: vec_score,
            });
        }
    }

    results.truncate(limit);
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    tracing::info!(result_count = results.len(), "search completed");
    Ok(Json(SearchResponse { results }))
}

pub async fn get_task_history(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TaskHistoryEntry>>, (StatusCode, String)> {
    let history = state.db.list_field_changes(id).await.map_err(|e| {
        tracing::error!(error = %e, "list field changes failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(history))
}
