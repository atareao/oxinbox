use axum::extract::{Path, State};
use axum::{Extension, Json, http::StatusCode, middleware};
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::auth::{AuthState, AuthUser};
use crate::core_types::Context;
use crate::middleware::require_auth;
use crate::repository::RepositoryError;

#[derive(Debug, Deserialize)]
pub struct CreateContextRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateContextRequest {
    pub name: Option<String>,
    pub color: Option<String>,
}

pub fn context_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route(
            "/api/contexts",
            axum::routing::get(list_contexts).post(create_context),
        )
        .route(
            "/api/contexts/{id}",
            axum::routing::get(get_context)
                .put(update_context)
                .delete(delete_context),
        )
        .layer(middleware::from_fn_with_state(state, require_auth))
}

#[instrument(skip(state, _user))]
pub async fn list_contexts(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
) -> Result<Json<Vec<Context>>, (StatusCode, String)> {
    let contexts = state.db.list_contexts().await.map_err(|e| {
        tracing::error!(error = %e, "db list contexts failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(contexts))
}

#[instrument(skip(state, _user))]
pub async fn create_context(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Json(req): Json<CreateContextRequest>,
) -> Result<Json<Context>, (StatusCode, String)> {
    let ctx = Context {
        id: Uuid::now_v7(),
        name: req.name,
        color: req.color,
        created_at: chrono::Utc::now(),
    };
    let created = state.db.create_context(&ctx).await.map_err(|e| {
        tracing::error!(error = %e, "db create context failed");
        (StatusCode::CONFLICT, e.to_string())
    })?;
    Ok(Json(created))
}

#[instrument(skip(state, _user), fields(context_id = %id))]
pub async fn get_context(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<Context>, (StatusCode, String)> {
    let ctx = state.db.get_context(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!(context_id = %id, "context not found");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "db get context failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;
    Ok(Json(ctx))
}

#[instrument(skip(state, _user), fields(context_id = %id))]
pub async fn update_context(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateContextRequest>,
) -> Result<Json<Context>, (StatusCode, String)> {
    let mut ctx = state.db.get_context(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!(context_id = %id, "context not found for update");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "db get context for update failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    if let Some(name) = req.name {
        ctx.name = name;
    }
    if let Some(color) = req.color {
        ctx.color = Some(color);
    }

    let updated = state.db.update_context(&ctx).await.map_err(|e| {
        tracing::error!(error = %e, "db update context failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    Ok(Json(updated))
}

#[instrument(skip(state, _user), fields(context_id = %id))]
pub async fn delete_context(
    State(state): State<AuthState>,
    Extension(_user): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.db.delete_context(id).await.map_err(|e| match e {
        RepositoryError::NotFound(_) => {
            tracing::warn!(context_id = %id, "context not found for deletion");
            (StatusCode::NOT_FOUND, e.to_string())
        }
        RepositoryError::Database(_) => {
            tracing::error!(error = %e, "db delete context failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;
    Ok(StatusCode::NO_CONTENT)
}
