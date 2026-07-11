use std::sync::Arc;

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use tracing::instrument;

use crate::ai::task_builder;
use crate::ai::{AiProvider, ChatMessage, ChatRole};
use crate::auth::AuthState;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct VoiceParams {
    pub token: String,
}

// ---------------------------------------------------------------------------
// WS handler
// ---------------------------------------------------------------------------

#[instrument(skip(state), fields(token = %params.token))]
pub async fn voice_handler(
    State(state): State<AuthState>,
    ws: WebSocketUpgrade,
    Query(params): Query<VoiceParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let claims = state
        .jwt_validator
        .validate_token(&params.token)
        .await
        .map_err(|e| {
            tracing::warn!("invalid voice WS token: {e}");
            (StatusCode::UNAUTHORIZED, "invalid token".into())
        })?;

    let ai_provider = state.ai_provider.clone().ok_or_else(|| {
        tracing::warn!("AI not configured for voice");
        (StatusCode::SERVICE_UNAVAILABLE, "AI not configured".into())
    })?;

    let user_id = claims.sub;
    tracing::info!(user_id, "voice WS upgrade");

    Ok(ws.on_upgrade(move |socket| handle_ws(socket, ai_provider, state.db.clone(), user_id)))
}

// ---------------------------------------------------------------------------
// WebSocket message loop
// ---------------------------------------------------------------------------

async fn handle_ws(
    mut socket: WebSocket,
    ai_provider: Arc<dyn AiProvider>,
    db: Arc<crate::database::ParadeDbRepository>,
    user_id: String,
) {
    let mut audio_buffer: Vec<u8> = Vec::new();

    /// Helper: serialise value and send as WS text message.
    async fn ws_send(socket: &mut WebSocket, value: &serde_json::Value) {
        let payload = value.to_string();
        if socket.send(Message::Text(payload.into())).await.is_err() {
            tracing::warn!("failed to send WS message (connection closed?)");
        }
    }

    loop {
        let Some(msg) = socket.recv().await else {
            tracing::debug!("voice WS closed");
            return;
        };

        let msg = match msg {
            Ok(msg) => msg,
            Err(e) => {
                tracing::error!(error = %e, "voice WS recv error");
                return;
            }
        };

        match msg {
            Message::Binary(data) => {
                audio_buffer.extend_from_slice(&data);
                tracing::trace!(
                    "received {} bytes (total {})",
                    data.len(),
                    audio_buffer.len()
                );
            }
            Message::Text(text) => {
                let text = text.trim().to_lowercase();

                // ---- cancel ----
                if text.contains("\"cancel\"") || text == "cancel" {
                    audio_buffer.clear();
                    tracing::info!("audio cancelled");
                    ws_send(&mut socket, &serde_json::json!({"type": "cancelled"})).await;
                    continue;
                }

                // ---- transcribe ----
                if text.contains("\"transcribe\"") || text == "transcribe" {
                    if audio_buffer.is_empty() {
                        ws_send(
                            &mut socket,
                            &serde_json::json!({"type": "error", "message": "no audio data"}),
                        )
                        .await;
                        continue;
                    }

                    let audio = std::mem::take(&mut audio_buffer);

                    // 1) Transcribe
                    ws_send(
                        &mut socket,
                        &serde_json::json!({"type": "status", "step": "transcribing"}),
                    )
                    .await;

                    let transcription = match ai_provider.transcribe(&audio, "audio/webm").await {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::error!(error = %e, "transcription failed");
                            ws_send(
                                &mut socket,
                                &serde_json::json!({"type": "error", "message": format!("transcription failed: {e}")}),
                            )
                            .await;
                            continue;
                        }
                    };

                    tracing::info!(text_len = transcription.len(), "transcription completed");

                    ws_send(
                        &mut socket,
                        &serde_json::json!({"type": "transcription", "text": transcription}),
                    )
                    .await;

                    // 2) Build enriched prompt (shared)
                    ws_send(
                        &mut socket,
                        &serde_json::json!({"type": "status", "step": "parsing"}),
                    )
                    .await;

                    // 2) Cargar configuración de prompts del usuario
                    let prompt_config = db.get_prompt_config(&user_id).await
                        .map_err(|e| {
                            tracing::warn!(error = %e, "failed to load prompt config, using defaults");
                            None::<crate::core_types::PromptConfig>
                        })
                        .ok()
                        .flatten();

                    let system_prompt = task_builder::build_task_prompt(
                        &db,
                        &user_id,
                        "transcripciones de voz",
                        prompt_config.as_ref(),
                    )
                    .await;

                    let llm_response = match ai_provider
                        .chat(
                            &system_prompt,
                            &[ChatMessage {
                                role: ChatRole::User,
                                content: transcription.clone(),
                            }],
                        )
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!(error = %e, "LLM parsing failed");
                            ws_send(
                                &mut socket,
                                &serde_json::json!({"type": "error", "message": format!("parse failed: {e}")}),
                            )
                            .await;
                            continue;
                        }
                    };

                    // 3) Parse LLM JSON response (shared)
                    let parsed = match task_builder::parse_llm_json(&llm_response) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!(error = %e, raw = %llm_response, "failed to parse LLM response");
                            ws_send(
                                &mut socket,
                                &serde_json::json!({"type": "parse_error", "raw": llm_response}),
                            )
                            .await;
                            continue;
                        }
                    };

                    // 4) Load projects/contexts for auto-assign fallback
                    let projects = db.list_projects().await.unwrap_or_default();
                    let contexts = db.list_contexts().await.unwrap_or_default();

                    // 5) Create task via shared function (with auto-assign fallback)
                    ws_send(
                        &mut socket,
                        &serde_json::json!({"type": "status", "step": "creating"}),
                    )
                    .await;

                    match task_builder::create_task_from_llm(
                        &db,
                        ai_provider.as_ref(),
                        &user_id,
                        parsed,
                        &projects,
                        &contexts,
                    )
                    .await
                    {
                        Ok(task) => {
                            tracing::info!(task_id = %task.id, "task created from voice");
                            ws_send(
                                &mut socket,
                                &serde_json::json!({"type": "task_created", "task": task}),
                            )
                            .await;
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "task creation failed");
                            ws_send(
                                &mut socket,
                                &serde_json::json!({"type": "error", "message": format!("task creation failed: {e}")}),
                            )
                            .await;
                        }
                    }

                    continue;
                }

                // ---- unknown ----
                tracing::warn!(unknown_msg = %text, "unknown WS command");
                ws_send(
                    &mut socket,
                    &serde_json::json!({"type": "error", "message": "unknown command"}),
                )
                .await;
            }
            Message::Close(_) => {
                tracing::debug!("voice WS close frame received");
                return;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Route factory
// ---------------------------------------------------------------------------

pub fn voice_routes() -> axum::Router<AuthState> {
    axum::Router::new().route("/api/voice", axum::routing::get(voice_handler))
}
