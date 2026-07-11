use axum::extract::{Multipart, State};
use axum::{Extension, Json, http::StatusCode, middleware};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::auth::{AuthState, AuthUser};
use crate::core_types::{Context, Project, Task, TaskStatus, Uuid};
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

#[derive(Debug, Deserialize)]
struct TextCaptureParsed {
    description: String,
    priority: Option<String>,
    project_name: Option<String>,
    context_name: Option<String>,
    due_date: Option<String>,
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
// Prompt builder (shared with voice WS)
// ---------------------------------------------------------------------------

pub async fn build_text_capture_prompt(
    db: &crate::database::ParadeDbRepository,
    user_id: &str,
) -> String {
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
                projects
                    .iter()
                    .find(|p| t.project_ids.contains(&p.id))
                    .map(|p| format!(" proyecto:{}", p.name))
                    .unwrap_or_default()
            };
            let ctx_str = if t.context_ids.is_empty() {
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
                ctx_str,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r##"Eres un asistente que convierte texto en tareas estructuradas.

Contexto actual:
- Proyectos existentes: {}
- Contextos existentes: {}
- Últimas tareas (para referencia de prioridades y patrones):
{}

Debes extraer UNA SOLA tarea del texto de usuario. Si hay múltiples, elige la más importante.

Analiza el texto y asigna prioridad según la urgencia implícita. Usa 'A' para muy urgente, 'B' para normal, 'C' para baja o null si no está claro.

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
// Task creator (shared with voice WS)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn create_task_from_parsed(
    db: &crate::database::ParadeDbRepository,
    ai: &dyn crate::ai::AiProvider,
    user_id: &str,
    description: String,
    priority: Option<char>,
    project_name: Option<String>,
    context_name: Option<String>,
    due_date: Option<String>,
) -> Result<Task, String> {
    let now = chrono::Utc::now();

    // Resolve project
    let project_ids = if let Some(name) = project_name {
        if name.is_empty() {
            vec![]
        } else {
            let project = db
                .create_or_find_project(&name)
                .await
                .map_err(|e| format!("failed to resolve project: {e}"))?;
            vec![project.id]
        }
    } else {
        vec![]
    };

    // Resolve context
    let context_ids = if let Some(name) = context_name {
        if name.is_empty() {
            vec![]
        } else {
            let ctx = db
                .create_or_find_context(&name)
                .await
                .map_err(|e| format!("failed to resolve context: {e}"))?;
            vec![ctx.id]
        }
    } else {
        vec![]
    };

    // Parse due_date
    let parsed_due = due_date
        .as_deref()
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

    let task = Task {
        id: Uuid::now_v7(),
        completed: false,
        priority: priority.filter(|c| ['A', 'B', 'C'].contains(c)),
        description,
        project_ids,
        context_ids,
        status: TaskStatus::Inbox,
        created_at: now,
        updated_at: now,
        completed_at: None,
        due_date: parsed_due,
    };

    let created = db
        .create(&task, user_id)
        .await
        .map_err(|e| format!("failed to create task: {e}"))?;

    // Generate embedding in background (best-effort)
    let embed_text = format!(
        "{} {}",
        created.description,
        created.priority.map_or(String::new(), |p| format!("({p})"))
    );
    if let Ok(embedding) = ai.embed(&embed_text).await {
        let _ = db.update_embedding(created.id, &embedding).await;
    }

    Ok(created)
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

    // 1) Build enriched prompt
    let system_prompt = build_text_capture_prompt(&state.db, &user.user_id).await;

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
    let clean = llm_response
        .trim()
        .strip_prefix("```json")
        .or_else(|| llm_response.trim().strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map_or_else(|| llm_response.trim(), str::trim);

    let parsed: TextCaptureParsed = serde_json::from_str(clean).map_err(|e| {
        tracing::error!(error = %e, raw = %clean, "failed to parse LLM response as JSON");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to parse LLM response: {e}"),
        )
    })?;

    // 4) Resolve and create
    let priority = parsed.priority.as_deref().and_then(|p| p.chars().next());
    let created = create_task_from_parsed(
        &state.db,
        ai.as_ref(),
        &user.user_id,
        parsed.description,
        priority,
        parsed.project_name,
        parsed.context_name,
        parsed.due_date,
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
