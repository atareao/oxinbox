use axum::extract::{Path, State};
use axum::{Extension, Json, http::StatusCode, middleware};
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::auth::{AuthState, AuthUser};
use crate::middleware::require_auth;
use crate::repository::RepositoryError;
use crate::core_types::Project;

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

pub fn project_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route(
            "/api/projects",
            axum::routing::get(list_projects).post(create_project),
        )
        .route(
            "/api/projects/{id}",
            axum::routing::get(get_project)
                .put(update_project)
                .delete(delete_project),
        )
        .layer(middleware::from_fn_with_state(state, require_auth))
}

#[instrument(skip(state, _user))]
pub async fn list_projects(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
) -> Result<Json<Vec<Project>>, (StatusCode, String)> {
    let projects = state.db.list_projects().await.map_err(|e| {
        tracing::error!(error = %e, "db list projects failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(projects))
}

#[instrument(skip(state, _user))]
pub async fn create_project(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<Project>, (StatusCode, String)> {
    let project = Project {
        id: Uuid::now_v7(),
        name: req.name,
        color: req.color,
        created_at: chrono::Utc::now(),
    };
    let created = state.db.create_project(&project).await.map_err(|e| {
        tracing::error!(error = %e, "db create project failed");
        (StatusCode::CONFLICT, e.to_string())
    })?;
    Ok(Json(created))
}

#[instrument(skip(state, _user), fields(project_id = %id))]
pub async fn get_project(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Project>, (StatusCode, String)> {
    let project = state.db.get_project(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!(project_id = %id, "project not found");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "db get project failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;
    Ok(Json(project))
}

#[instrument(skip(state, _user), fields(project_id = %id))]
pub async fn update_project(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<Project>, (StatusCode, String)> {
    let mut project = state.db.get_project(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!(project_id = %id, "project not found for update");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "db get project for update failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    if let Some(name) = req.name {
        project.name = name;
    }
    if let Some(color) = req.color {
        project.color = Some(color);
    }

    let updated = state.db.update_project(&project).await.map_err(|e| {
        tracing::error!(error = %e, "db update project failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(updated))
}

#[instrument(skip(state, _user), fields(project_id = %id))]
pub async fn delete_project(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.db.delete_project(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!(project_id = %id, "project not found for deletion");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "db delete project failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;
    Ok(StatusCode::NO_CONTENT)
}