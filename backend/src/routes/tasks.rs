use axum::extract::State;
use axum::{Extension, Json, extract::Path, http::StatusCode, middleware};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::auth::{AuthState, AuthUser};
use crate::middleware::require_auth;
use crate::repository::{self, InMemoryTaskRepository, RepositoryError, TaskRepository};
use crate::search::SearchResult;
use oxinbox_core::Task;

pub fn task_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route(
            "/api/tasks",
            axum::routing::get(list_tasks).post(create_task),
        )
        .route(
            "/api/tasks/:id",
            axum::routing::get(get_task)
                .put(update_task)
                .delete(delete_task),
        )
        .route("/api/tasks/search", axum::routing::post(search_tasks))
        .layer(middleware::from_fn_with_state(state, require_auth))
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub description: String,
    pub priority: Option<char>,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
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

fn task_embedding_text(task: &Task) -> String {
    let mut parts = vec![task.description.clone()];
    for p in &task.projects {
        parts.push(format!("+{p}"));
    }
    for c in &task.contexts {
        parts.push(format!("@{c}"));
    }
    parts.join(" ")
}

async fn generate_and_store_embedding(
    ai: &dyn crate::ai::AiProvider,
    task: &Task,
    db: Option<&crate::database::ParadeDbRepository>,
) {
    let text = task_embedding_text(task);
    match ai.embed(&text).await {
        Ok(embedding) => {
            repository::store_embedding(task.id, embedding.clone()).await;
            if let Some(database) = db {
                let _ = database.update_embedding(task.id, &embedding).await;
            }
            tracing::debug!(task_id = %task.id, "embedding stored");
        }
        Err(e) => {
            tracing::warn!(task_id = %task.id, error = %e, "embedding generation failed");
        }
    }
}

#[instrument(skip(state), fields(description = %req.description))]
pub async fn create_task(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let _ = user;
    let now = chrono::Utc::now();
    let task = Task {
        id: Uuid::now_v7(),
        completed: false,
        priority: req.priority.filter(char::is_ascii_uppercase),
        description: req.description,
        projects: req.projects,
        contexts: req.contexts,
        status: oxinbox_core::TaskStatus::Inbox,
        created_at: now,
        updated_at: now,
        completed_at: None,
        due_date: req.due_date,
    };

    let created = if let Some(db) = &state.db {
        db.create(&task, user.user_id).await.map_err(|e| {
            tracing::error!(error = %e, "db create failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
    } else {
        let repo = InMemoryTaskRepository::shared();
        repo.create(&task).await.map_err(|e| {
            tracing::error!(error = %e, "create task failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
    };

    if let Some(ai) = &state.ai_provider {
        generate_and_store_embedding(ai.as_ref(), &task, state.db.as_deref()).await;
    }

    tracing::info!(task_id = %created.id, "task created");
    Ok(Json(created))
}

#[instrument(skip(state))]
pub async fn list_tasks(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<Task>>, (StatusCode, String)> {
    let tasks = if let Some(db) = &state.db {
        db.list(user.user_id).await.map_err(|e| {
            tracing::error!(error = %e, "db list failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
    } else {
        let repo = InMemoryTaskRepository::shared();
        repo.list(0).await.map_err(|e| {
            tracing::error!(error = %e, "list tasks failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
    };
    tracing::info!(count = tasks.len(), "tasks listed");
    Ok(Json(tasks))
}

#[instrument(skip(state), fields(task_id = %id))]
pub async fn get_task(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let task = if let Some(db) = &state.db {
        db.get(id).await.map_err(|e| match e {
            RepositoryError::NotFound(_) => {
                tracing::warn!("task not found");
                (StatusCode::NOT_FOUND, e.to_string())
            }
            RepositoryError::Database(_) => {
                tracing::error!(error = %e, "get task database error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?
    } else {
        let repo = InMemoryTaskRepository::shared();
        repo.get(id).await.map_err(|e| match e {
            RepositoryError::NotFound(_) => {
                tracing::warn!("task not found");
                (StatusCode::NOT_FOUND, e.to_string())
            }
            RepositoryError::Database(_) => {
                tracing::error!(error = %e, "get task database error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?
    };
    Ok(Json(task))
}

#[instrument(skip(state, _user), fields(task_id = %id))]
pub async fn update_task(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(updates): Json<CreateTaskRequest>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let mut task = if let Some(db) = &state.db {
        db.get(id).await.map_err(|e| match e {
            RepositoryError::NotFound(_) => {
                tracing::warn!("task not found for update");
                (StatusCode::NOT_FOUND, e.to_string())
            }
            RepositoryError::Database(_) => {
                tracing::error!(error = %e, "get task database error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?
    } else {
        let repo = InMemoryTaskRepository::shared();
        repo.get(id).await.map_err(|e| match e {
            RepositoryError::NotFound(_) => {
                tracing::warn!("task not found for update");
                (StatusCode::NOT_FOUND, e.to_string())
            }
            RepositoryError::Database(_) => {
                tracing::error!(error = %e, "get task database error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?
    };

    task.description = updates.description;
    task.priority = updates.priority.filter(char::is_ascii_uppercase);
    task.projects = updates.projects;
    task.contexts = updates.contexts;
    task.due_date = updates.due_date;

    if let Some(status_str) = &updates.status {
        task.status = match status_str.to_lowercase().as_str() {
            "inbox" => oxinbox_core::TaskStatus::Inbox,
            "todo" => oxinbox_core::TaskStatus::Todo,
            "doing" => oxinbox_core::TaskStatus::Doing,
            "done" => oxinbox_core::TaskStatus::Done,
            "someday" => oxinbox_core::TaskStatus::Someday,
            _ => task.status,
        };
    }

    let result = if let Some(db) = &state.db {
        db.update(&task).await.map_err(|e| {
            tracing::error!(error = %e, "db update failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
    } else {
        let repo = InMemoryTaskRepository::shared();
        repo.update(&task).await.map_err(|e| {
            tracing::error!(error = %e, "update task failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?
    };

    if let Some(ai) = &state.ai_provider {
        generate_and_store_embedding(ai.as_ref(), &task, state.db.as_deref()).await;
    }

    tracing::info!("task updated");
    Ok(Json(result))
}

#[instrument(skip(state), fields(task_id = %id))]
pub async fn delete_task(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if let Some(db) = &state.db {
        db.delete(id).await.map_err(|e| match e {
            RepositoryError::NotFound(_) => {
                tracing::warn!("task not found for deletion");
                (StatusCode::NOT_FOUND, e.to_string())
            }
            RepositoryError::Database(_) => {
                tracing::error!(error = %e, "delete task database error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?;
    } else {
        let repo = InMemoryTaskRepository::shared();
        repo.delete(id).await.map_err(|e| match e {
            RepositoryError::NotFound(_) => {
                tracing::warn!("task not found for deletion");
                (StatusCode::NOT_FOUND, e.to_string())
            }
            RepositoryError::Database(_) => {
                tracing::error!(error = %e, "delete task database error");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
        })?;
    }
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
    let (_tasks, results) = if let Some(db) = &state.db {
        let bm25_tasks = db
            .search_bm25(user.user_id, &req.query, limit_i64)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "db search failed");
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;

        let vector_results = if let Some(emb) = query_embedding {
            db.search_vector(user.user_id, &emb, limit_i64)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();

        for task in bm25_tasks {
            if seen.insert(task.id) {
                results.push(crate::search::SearchResult { task, score: 1.0 });
            }
        }
        for (task, score) in vector_results {
            if seen.insert(task.id) {
                results.push(crate::search::SearchResult { task, score });
            }
        }

        results.truncate(limit);
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        (Vec::new(), results)
    } else {
        let repo = InMemoryTaskRepository::shared();
        let tasks = repo.list(0).await.map_err(|e| {
            tracing::error!(error = %e, "list tasks for search failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        let results =
            repository::hybrid_search_tasks(&tasks, &req.query, query_embedding.as_deref(), limit)
                .await;
        (tasks, results)
    };

    tracing::info!(result_count = results.len(), "search completed");
    Ok(Json(SearchResponse { results }))
}
