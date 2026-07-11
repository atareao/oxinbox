use axum::{Extension, Json, extract::State};
use axum::http::StatusCode;
use serde::Deserialize;
use tracing::instrument;

use crate::auth::{AuthState, AuthUser};
use crate::core_types::PromptConfig;
use crate::ai::task_builder::{DEFAULT_SYSTEM_INSTRUCTIONS, DEFAULT_FEW_SHOT_EXAMPLES, DEFAULT_RULES};

#[derive(Debug, Deserialize)]
pub struct UpdatePromptRequest {
    pub system_instructions: String,
    pub few_shot_examples: String,
    pub rules: String,
}

pub fn prompt_routes(state: AuthState) -> axum::Router<AuthState> {
    use axum::routing::get;
    use axum::middleware;

    axum::Router::new()
        .route(
            "/api/prompts",
            get(get_prompt_config).put(update_prompt_config),
        )
        .layer(middleware::from_fn_with_state(state, crate::middleware::require_auth))
}

#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn get_prompt_config(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<PromptConfig>, (StatusCode, String)> {
    let config = state
        .db
        .get_prompt_config(&user.user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch prompt config");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;

    // Return defaults if user hasn't saved any config yet
    let config = config.unwrap_or_else(|| PromptConfig {
        user_id: user.user_id.clone(),
        system_instructions: DEFAULT_SYSTEM_INSTRUCTIONS.to_string(),
        few_shot_examples: DEFAULT_FEW_SHOT_EXAMPLES.to_string(),
        rules: DEFAULT_RULES.to_string(),
        updated_at: chrono::Utc::now(),
    });

    Ok(Json(config))
}

#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn update_prompt_config(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<UpdatePromptRequest>,
) -> Result<Json<PromptConfig>, (StatusCode, String)> {
    let config = PromptConfig {
        user_id: user.user_id.clone(),
        system_instructions: req.system_instructions,
        few_shot_examples: req.few_shot_examples,
        rules: req.rules,
        updated_at: chrono::Utc::now(),
    };

    let saved = state.db.upsert_prompt_config(&config).await.map_err(|e| {
        tracing::error!(error = %e, "failed to save prompt config");
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    tracing::info!(user_id = %user.user_id, "prompt config updated");
    Ok(Json(saved))
}