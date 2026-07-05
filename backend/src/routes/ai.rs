use axum::extract::{Multipart, State};
use axum::{Extension, Json, http::StatusCode, middleware};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::auth::{AuthState, AuthUser};
use crate::middleware::require_auth;

#[derive(Serialize)]
pub struct TranscribeResponse {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ParseTaskRequest {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedTask {
    pub description: String,
    pub priority: Option<char>,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
    pub due_date: Option<String>,
}

pub fn ai_routes(state: AuthState) -> axum::Router<AuthState> {
    axum::Router::new()
        .route("/api/transcribe", axum::routing::post(transcribe))
        .route("/api/parse-task", axum::routing::post(parse_task))
        .layer(middleware::from_fn_with_state(state, require_auth))
}

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

#[instrument(skip(state, user), fields(user_id = %user.user_id, text_len = req.text.len()))]
pub async fn parse_task(
    State(state): State<AuthState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<ParseTaskRequest>,
) -> Result<Json<ParsedTask>, (StatusCode, String)> {
    let ai = state.ai_provider.as_ref().ok_or_else(|| {
        tracing::warn!(user_id = %user.user_id, "AI not configured");
        (StatusCode::SERVICE_UNAVAILABLE, "AI not configured".into())
    })?;

    let system_prompt = concat!(
        "Eres un asistente que extrae tareas de texto natural al formato todo.txt estructurado.\n\n",
        "El usuario te dará una descripción en lenguaje natural de una tarea. Debes extraer:\n",
        "- `description`: La descripción limpia de la tarea (sin proyectos, contextos ni prioridad)\n",
        "- `priority`: Prioridad (A, B, C) o null si no se especifica\n",
        "- `projects`: Lista de proyectos (palabras que empiezan con +)\n",
        "- `contexts`: Lista de contextos (palabras que empiezan con @)\n",
        "- `due_date`: Fecha de vencimiento en formato YYYY-MM-DD o null\n\n",
        "Responde ÚNICAMENTE con un objeto JSON válido, sin texto adicional."
    );

    let messages = vec![crate::ai::ChatMessage {
        role: crate::ai::ChatRole::User,
        content: req.text,
    }];

    let result = ai.chat(system_prompt, &messages).await.map_err(|e| {
        tracing::error!(error = %e, "parse task failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse task failed: {e}"),
        )
    })?;

    let parsed: ParsedTask = serde_json::from_str(&result).map_err(|e| {
        tracing::error!(error = %e, raw = %result, "failed to parse LLM response as JSON");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to parse task: {e}"),
        )
    })?;

    tracing::info!(description = %parsed.description, "task parsed");
    Ok(Json(parsed))
}
