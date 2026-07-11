use axum::extract::{Multipart, State};
use axum::{Extension, Json, http::StatusCode, middleware};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::ai::task_builder;
use crate::auth::{AuthState, AuthUser};
use crate::core_types::{Context, Project, Task, Uuid};
use crate::middleware::require_auth;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct TranscribeResponse {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct TextCaptureRequest {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ResolveRequest {
    pub description: String,
    pub priority: Option<char>,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
    pub due_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResolveResponse {
    pub description: String,
    pub priority: Option<char>,
    pub project_ids: Vec<Uuid>,
    pub context_ids: Vec<Uuid>,
    pub due_date: Option<String>,
    pub resolved_projects: Vec<Project>,
    pub resolved_contexts: Vec<Context>,
}

// ---------------------------------------------------------------------------
// Route factory
// ---------------------------------------------------------------------------

pub fn ai_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route("/api/transcribe", axum::routing::post(transcribe))
        .route("/api/text-capture", axum::routing::post(text_capture))
        .route("/api/ai/resolve", axum::routing::post(resolve_task))
        .layer(middleware::from_fn_with_state(state, require_auth))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[instrument(skip(state, user, multipart), fields(user_id = %user.user_id))]
pub async fn transcribe(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    mut multipart: Multipart,
) -> Result<Json<TranscribeResponse>, (StatusCode, String)> {
    let ai = state.ai_provider.as_ref().ok_or_else(|| {
        tracing::warn!(user_id = %user.user_id, "AI not configured");
        (StatusCode::SERVICE_UNAVAILABLE, "AI not configured".into())
    })?;

    let mut audio_data = Vec::new();
    let mut mime_type = "audio/webm".to_string();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!(error = %e, "invalid multipart");
        (StatusCode::BAD_REQUEST, format!("invalid multipart: {e}"))
    })? {
        let name = field.name().unwrap_or("").to_string();
        if name == "audio" {
            mime_type = field.content_type().unwrap_or("audio/webm").to_string();
            audio_data = field
                .bytes()
                .await
                .map_err(|e| {
                    tracing::warn!(error = %e, "failed to read audio field");
                    (
                        StatusCode::BAD_REQUEST,
                        format!("failed to read audio: {e}"),
                    )
                })?
                .to_vec();
        }
    }

    if audio_data.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "no audio field found in multipart request".into(),
        ));
    }

    tracing::info!(bytes = audio_data.len(), mime = %mime_type, "transcribing audio");

    let text = ai.transcribe(&audio_data, &mime_type).await.map_err(|e| {
        tracing::error!(error = %e, "transcription failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("transcription failed: {e}"),
        )
    })?;

    tracing::info!(text_len = text.len(), "transcription completed");
    Ok(Json(TranscribeResponse { text }))
}

#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn text_capture(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<TextCaptureRequest>,
) -> Result<Json<Task>, (StatusCode, String)> {
    let ai = state.ai_provider.as_ref().ok_or_else(|| {
        tracing::warn!("AI not configured for text-capture");
        (StatusCode::SERVICE_UNAVAILABLE, "AI not configured".into())
    })?;

    // 1) Cargar configuración de prompts del usuario (si existe)
    let prompt_config = state
        .db
        .get_prompt_config(&user.user_id)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "failed to load prompt config, using defaults");
            None::<crate::core_types::PromptConfig>
        })
        .ok()
        .flatten();

    // 2) Build enriched prompt with optional config
    let system_prompt =
        task_builder::build_task_prompt(&state.db, &user.user_id, "texto", prompt_config.as_ref())
            .await;

    // 2) Call LLM
    let llm_response = ai
        .chat(
            &system_prompt,
            &[crate::ai::ChatMessage {
                role: crate::ai::ChatRole::User,
                content: req.text.clone(),
            }],
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "LLM text-capture failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("LLM parse failed: {e}"),
            )
        })?;

    // 3) Parse LLM JSON response
    let parsed = task_builder::parse_llm_json(&llm_response).map_err(|e| {
        tracing::error!(error = %e, raw = %llm_response, "failed to parse LLM response");
        (StatusCode::INTERNAL_SERVER_ERROR, e)
    })?;

    // 4) Load projects/contexts for auto-assign fallback
    let projects = state.db.list_projects().await.unwrap_or_default();
    let contexts = state.db.list_contexts().await.unwrap_or_default();

    // 5) Create task (with auto-assign fallback if LLM omitted project/context)
    let created = task_builder::create_task_from_llm(
        &state.db,
        ai.as_ref(),
        &user.user_id,
        parsed,
        &projects,
        &contexts,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "task creation from text-capture failed");
        (StatusCode::INTERNAL_SERVER_ERROR, e)
    })?;

    tracing::info!(task_id = %created.id, "task created from text-capture");
    Ok(Json(created))
}

#[instrument(skip(state, user), fields(user_id = %user.user_id))]
pub async fn resolve_task(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<ResolveRequest>,
) -> Result<Json<ResolveResponse>, (StatusCode, String)> {
    let _ = user;

    let mut resolved_projects = Vec::new();
    let mut project_ids = Vec::new();
    for name in &req.projects {
        let project = state.db.create_or_find_project(name).await.map_err(|e| {
            tracing::error!(error = %e, "resolve project failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        project_ids.push(project.id);
        resolved_projects.push(project);
    }

    let mut resolved_contexts = Vec::new();
    let mut context_ids = Vec::new();
    for name in &req.contexts {
        let ctx = state.db.create_or_find_context(name).await.map_err(|e| {
            tracing::error!(error = %e, "resolve context failed");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        context_ids.push(ctx.id);
        resolved_contexts.push(ctx);
    }

    Ok(Json(ResolveResponse {
        description: req.description,
        priority: req.priority,
        project_ids,
        context_ids,
        due_date: req.due_date,
        resolved_projects,
        resolved_contexts,
    }))
}
