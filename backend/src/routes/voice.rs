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

use crate::ai::{AiProvider, ChatMessage, ChatRole};
use crate::auth::AuthState;

#[derive(Debug, Deserialize)]
pub struct VoiceParams {
    pub token: String,
}

#[instrument(skip(state), fields(token = %params.token))]
pub async fn voice_handler(
    State(state): State<AuthState>,
    ws: WebSocketUpgrade,
    Query(params): Query<VoiceParams>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user_id = state
        .sessions
        .read()
        .await
        .get(&params.token)
        .copied()
        .ok_or_else(|| {
            tracing::warn!("invalid voice WS token");
            (StatusCode::UNAUTHORIZED, "invalid token".into())
        })?;

    let ai_provider = state.ai_provider.clone().ok_or_else(|| {
        tracing::warn!("AI not configured for voice");
        (StatusCode::SERVICE_UNAVAILABLE, "AI not configured".into())
    })?;

    tracing::info!(user_id, "voice WS upgrade");
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, ai_provider)))
}

const PARSE_SYSTEM_PROMPT: &str = concat!(
    "Eres un asistente que parsea transcripciones de voz a tareas en formato todo.txt.\n\n",
    "Extrae del texto: description, priority (A/B/C), projects (+proyecto), contexts (@contexto), due_date (YYYY-MM-DD).\n\n",
    "Responde ÚNICAMENTE con un JSON sin markdown:\n",
    "{\n",
    "  \"description\": \"...\",\n",
    "  \"priority\": \"A\" | null,\n",
    "  \"projects\": [\"...\"],\n",
    "  \"contexts\": [\"...\"],\n",
    "  \"due_date\": \"YYYY-MM-DD\" | null\n",
    "}",
);

#[derive(Deserialize)]
struct ParsedTaskInput {
    description: String,
    priority: Option<String>,
    projects: Vec<String>,
    contexts: Vec<String>,
    due_date: Option<String>,
}

#[allow(clippy::too_many_lines)]
async fn handle_ws(mut socket: WebSocket, ai_provider: Arc<dyn AiProvider>) {
    let mut audio_buffer: Vec<u8> = Vec::new();

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

                if text.contains("\"cancel\"") || text == "cancel" {
                    audio_buffer.clear();
                    tracing::info!("audio cancelled");
                    let _ = socket
                        .send(Message::Text(r#"{"type":"cancelled"}"#.into()))
                        .await;
                    continue;
                }

                if text.contains("\"transcribe\"") || text == "transcribe" {
                    if audio_buffer.is_empty() {
                        let _ = socket
                            .send(Message::Text(
                                r#"{"type":"error","message":"no audio data"}"#.into(),
                            ))
                            .await;
                        continue;
                    }

                    let audio = std::mem::take(&mut audio_buffer);
                    tracing::info!(bytes = audio.len(), "transcribing audio");

                    let transcription = ai_provider.transcribe(&audio, "audio/webm").await;

                    match transcription {
                        Ok(transcribed_text) => {
                            let _ = socket
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "transcription",
                                        "text": transcribed_text
                                    })
                                    .to_string(),
                                ))
                                .await;

                            let parse_result = ai_provider
                                .chat(
                                    PARSE_SYSTEM_PROMPT,
                                    &[ChatMessage {
                                        role: ChatRole::User,
                                        content: transcribed_text,
                                    }],
                                )
                                .await;

                            match parse_result {
                                Ok(json_str) => {
                                    let clean = json_str
                                        .trim()
                                        .strip_prefix("```json")
                                        .or_else(|| json_str.trim().strip_prefix("```"))
                                        .and_then(|s| s.strip_suffix("```"))
                                        .map_or_else(|| json_str.trim(), str::trim);

                                    match serde_json::from_str::<ParsedTaskInput>(clean) {
                                        Ok(parsed) => {
                                            let _ = socket
                                                .send(Message::Text(
                                                    serde_json::json!({
                                                        "type": "task",
                                                        "task": {
                                                            "description": parsed.description,
                                                            "priority": parsed.priority,
                                                            "projects": parsed.projects,
                                                            "contexts": parsed.contexts,
                                                            "due_date": parsed.due_date,
                                                        }
                                                    })
                                                    .to_string(),
                                                ))
                                                .await;
                                        }
                                        Err(e) => {
                                            tracing::warn!(error = %e, raw = %clean, "failed to parse LLM output as task JSON");
                                            let _ = socket
                                                .send(Message::Text(
                                                    serde_json::json!({
                                                        "type": "parse_error",
                                                        "raw": json_str
                                                    })
                                                    .to_string(),
                                                ))
                                                .await;
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "LLM parse failed");
                                    let _ = socket
                                        .send(Message::Text(
                                            serde_json::json!({
                                                "type": "error",
                                                "message": format!("parse failed: {e}")
                                            })
                                            .to_string(),
                                        ))
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "transcription failed");
                            let _ = socket
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "error",
                                        "message": format!("transcription failed: {e}")
                                    })
                                    .to_string(),
                                ))
                                .await;
                        }
                    }
                    continue;
                }

                tracing::warn!(unknown_msg = %text, "unknown WS command");
                let _ = socket
                    .send(Message::Text(
                        r#"{"type":"error","message":"unknown command"}"#.into(),
                    ))
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

pub fn voice_routes() -> axum::Router<AuthState> {
    axum::Router::new().route("/api/voice", axum::routing::get(voice_handler))
}
