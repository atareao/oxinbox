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
use crate::core_types::{Task, TaskStatus, Uuid};
use crate::database::ParadeDbRepository;

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct VoiceParams {
    pub token: String,
}

// ---------------------------------------------------------------------------
// LLM response shape
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct VoiceParsedTask {
    description: String,
    priority: Option<String>,
    project_name: Option<String>,
    context_name: Option<String>,
    due_date: Option<String>,
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
// Prompt builder
// ---------------------------------------------------------------------------

async fn build_enriched_system_prompt(db: &ParadeDbRepository, user_id: &str) -> String {
    let projects = db.list_projects().await.unwrap_or_default();
    let contexts = db.list_contexts().await.unwrap_or_default();
    let last_tasks = db.list(user_id).await.unwrap_or_default();

    let project_names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
    let context_names: Vec<&str> = contexts.iter().map(|c| c.name.as_str()).collect();

    let tasks_summary: String = last_tasks
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, t)| {
            let prio = t.priority.map_or(String::new(), |p| format!(" [{p}]"));
            let proj = if t.project_ids.is_empty() {
                String::new()
            } else {
                // Try to find project name by ID (best-effort)
                projects
                    .iter()
                    .find(|p| t.project_ids.contains(&p.id))
                    .map(|p| format!(" proyecto:{},", p.name))
                    .unwrap_or_default()
            };
            let ctx_name = if t.context_ids.is_empty() {
                String::new()
            } else {
                contexts
                    .iter()
                    .find(|c| t.context_ids.contains(&c.id))
                    .map(|c| format!(" contexto:{}", c.name))
                    .unwrap_or_default()
            };
            let status = format!("{:?}", t.status).to_lowercase();
            format!(
                "  {}.{prio} {} ({}{}) — {status}",
                i + 1,
                t.description,
                proj,
                ctx_name,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r##"Eres un asistente que convierte transcripciones de voz en tareas estructuradas.

Contexto actual:
- Proyectos existentes: {}
- Contextos existentes: {}
- Últimas tareas (para referencia de prioridades y patrones):
{}

Debes extraer UNA SOLA tarea del texto de usuario. Si hay múltiples, elige la más importante.

Analiza la transcripción y asigna prioridad según la urgencia implícita. Usa 'A' para muy urgente, 'B' para normal, 'C' para baja o null si no está claro.

Para project_name y context_name: si existe una coincidencia clara con los existentes, úsala. Si no existe, SUGIERE un nombre nuevo y el sistema lo creará automáticamente. Si no aplica ninguno, devuelve null.

Responde ÚNICAMENTE con un JSON válido, sin markdown, sin texto adicional:
{{
  "description": "Descripción limpia de la tarea",
  "priority": "A" | "B" | "C" | null,
  "project_name": "Nombre del proyecto" | null,
  "context_name": "Nombre del contexto" | null,
  "due_date": "YYYY-MM-DD" | null
}}"##,
        serde_json::to_string(&project_names).unwrap_or_default(),
        serde_json::to_string(&context_names).unwrap_or_default(),
        tasks_summary,
    )
}

// ---------------------------------------------------------------------------
// Task creator
// ---------------------------------------------------------------------------

#[allow(clippy::cast_possible_truncation)]
async fn create_task_from_voice(
    db: &ParadeDbRepository,
    ai: &dyn AiProvider,
    user_id: &str,
    parsed: VoiceParsedTask,
) -> Result<Task, String> {
    let now = chrono::Utc::now();

    // Resolve project
    let project_ids = if let Some(name) = &parsed.project_name {
        if name.is_empty() {
            vec![]
        } else {
            let project = db
                .create_or_find_project(name)
                .await
                .map_err(|e| format!("failed to resolve project: {e}"))?;
            vec![project.id]
        }
    } else {
        vec![]
    };

    // Resolve context
    let context_ids = if let Some(name) = &parsed.context_name {
        if name.is_empty() {
            vec![]
        } else {
            let ctx = db
                .create_or_find_context(name)
                .await
                .map_err(|e| format!("failed to resolve context: {e}"))?;
            vec![ctx.id]
        }
    } else {
        vec![]
    };

    // Parse priority
    let priority = parsed
        .priority
        .as_deref()
        .and_then(|p| p.chars().next())
        .filter(|c| c.is_ascii_uppercase() && ['A', 'B', 'C'].contains(c));

    // Parse due_date
    let due_date = parsed.due_date.as_deref().and_then(|d| {
        // Try full ISO first, then just date
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .or_else(|| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ").ok())
    });

    let task = Task {
        id: Uuid::now_v7(),
        completed: false,
        priority,
        description: parsed.description,
        project_ids,
        context_ids,
        status: TaskStatus::Inbox,
        created_at: now,
        updated_at: now,
        completed_at: None,
        due_date,
    };

    let created = db
        .create(&task, user_id)
        .await
        .map_err(|e| format!("failed to create task: {e}"))?;

    // Generate embedding in background (best-effort)
    let text = format!(
        "{} {}",
        created.description,
        created.priority.map_or(String::new(), |p| format!("({p})"))
    );
    if let Ok(embedding) = ai.embed(&text).await {
        let _ = db.update_embedding(created.id, &embedding).await;
    }

    tracing::info!(task_id = %created.id, "task created from voice");
    Ok(created)
}

// ---------------------------------------------------------------------------
// WebSocket message loop
// ---------------------------------------------------------------------------

async fn handle_ws(
    mut socket: WebSocket,
    ai_provider: Arc<dyn AiProvider>,
    db: Arc<ParadeDbRepository>,
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

                    // 2) Build enriched prompt
                    ws_send(
                        &mut socket,
                        &serde_json::json!({"type": "status", "step": "parsing"}),
                    )
                    .await;

                    let system_prompt = build_enriched_system_prompt(&db, &user_id).await;

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

                    // 3) Parse LLM JSON response
                    let clean = llm_response
                        .trim()
                        .strip_prefix("```json")
                        .or_else(|| llm_response.trim().strip_prefix("```"))
                        .and_then(|s| s.strip_suffix("```"))
                        .map_or_else(|| llm_response.trim(), str::trim);

                    let parsed: VoiceParsedTask = match serde_json::from_str(clean) {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!(error = %e, raw = %clean, "failed to parse LLM response as JSON");
                            ws_send(
                                &mut socket,
                                &serde_json::json!({"type": "parse_error", "raw": llm_response}),
                            )
                            .await;
                            continue;
                        }
                    };

                    // 4) Create task
                    ws_send(
                        &mut socket,
                        &serde_json::json!({"type": "status", "step": "creating"}),
                    )
                    .await;

                    match create_task_from_voice(&db, ai_provider.as_ref(), &user_id, parsed).await
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
