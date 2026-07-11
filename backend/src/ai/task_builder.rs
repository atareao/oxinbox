use serde::Deserialize;

use crate::core_types::{Context, Project, Task, TaskStatus, Uuid};
use crate::database::ParadeDbRepository;

use super::{AiProvider, ChatMessage, ChatRole};

// ---------------------------------------------------------------------------
// Parsed task from LLM (unifies TextCaptureParsed + VoiceParsedTask)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LlmParsedTask {
    pub description: String,
    pub priority: Option<String>,
    pub project_name: Option<String>,
    pub context_name: Option<String>,
    pub due_date: Option<String>,
}

// ---------------------------------------------------------------------------
// Prompt builder — shared between text-capture and voice
// ---------------------------------------------------------------------------

/// Build the LLM system prompt for parsing a task.
///
/// `source` describes the input format — either `"texto"` for text-capture
/// or `"transcripciones de voz"` for voice.
pub async fn build_task_prompt(db: &ParadeDbRepository, user_id: &str, source: &str) -> String {
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
        r##"Eres un asistente que convierte {source} en tareas estructuradas.

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
// LLM JSON parser — strips markdown fences
// ---------------------------------------------------------------------------

/// Parse a raw LLM JSON response, stripping markdown code fences if present.
pub fn parse_llm_json(raw: &str) -> Result<LlmParsedTask, String> {
    let clean = raw
        .trim()
        .strip_prefix("```json")
        .or_else(|| raw.trim().strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map_or_else(|| raw.trim(), str::trim);

    serde_json::from_str::<LlmParsedTask>(clean)
        .map_err(|e| format!("failed to parse LLM response as JSON: {e}: {clean}"))
}

// ---------------------------------------------------------------------------
// Auto-assign projects/contexts via LLM (fallback)
// ---------------------------------------------------------------------------

/// If the LLM didn't assign any project/context, fall back to auto-assignment
/// based on description similarity.
pub async fn auto_assign_context_fallback(
    ai: &dyn AiProvider,
    description: &str,
    projects: &[Project],
    contexts: &[Context],
    project_name: Option<String>,
    context_name: Option<String>,
) -> (Vec<Uuid>, Vec<Uuid>) {
    let mut project_ids = Vec::new();
    let mut context_ids = Vec::new();

    // If LLM already provided names, resolve them (no fallback needed)
    if let Some(ref name) = project_name
        && !name.is_empty()
    {
        // Will be resolved by caller — signal that we shouldn't auto-assign
        return (vec![], vec![]);
    }
    if let Some(ref name) = context_name
        && !name.is_empty()
    {
        return (vec![], vec![]);
    }

    // Auto-assign only if there's something to assign
    if projects.is_empty() && contexts.is_empty() {
        return (vec![], vec![]);
    }

    let project_names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
    let context_names: Vec<&str> = contexts.iter().map(|c| c.name.as_str()).collect();

    let system_prompt = format!(
        concat!(
            "Eres un asistente que asigna proyectos y contextos a tareas.\n\n",
            "Proyectos disponibles: {}\n",
            "Contextos disponibles: {}\n\n",
            "Responde ÚNICAMENTE con un JSON: {{\"projects\":[],\"contexts\":[]}}\n",
            "Selecciona solo los que más se relacionen con la tarea. Si ninguno, listas vacías."
        ),
        serde_json::to_string(&project_names).unwrap_or_default(),
        serde_json::to_string(&context_names).unwrap_or_default()
    );

    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: description.to_string(),
    }];

    if let Ok(result) = ai.chat(&system_prompt, &messages).await {
        #[derive(Deserialize)]
        struct AutoAssignResponse {
            projects: Vec<String>,
            contexts: Vec<String>,
        }

        if let Ok(parsed) = serde_json::from_str::<AutoAssignResponse>(&result) {
            project_ids = projects
                .iter()
                .filter(|p| parsed.projects.contains(&p.name))
                .map(|p| p.id)
                .collect();
            context_ids = contexts
                .iter()
                .filter(|c| parsed.contexts.contains(&c.name))
                .map(|c| c.id)
                .collect();

            if !project_ids.is_empty() || !context_ids.is_empty() {
                tracing::info!(
                    project_ids = ?project_ids,
                    context_ids = ?context_ids,
                    "auto-assign fallback applied"
                );
            }
        }
    }

    (project_ids, context_ids)
}

// ---------------------------------------------------------------------------
// Task creator — shared between text-capture and voice
// ---------------------------------------------------------------------------

/// Create a task from an LLM-parsed description.
///
/// Resolves project/context names (create-or-find), creates the task,
/// persists it, and generates an embedding in the background.
#[allow(clippy::too_many_arguments)]
pub async fn create_task_from_llm(
    db: &ParadeDbRepository,
    ai: &dyn AiProvider,
    user_id: &str,
    parsed: LlmParsedTask,
    projects: &[Project],
    contexts: &[Context],
) -> Result<Task, String> {
    let now = chrono::Utc::now();

    // Resolve project
    let project_ids = if let Some(ref name) = parsed.project_name {
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
    let context_ids = if let Some(ref name) = parsed.context_name {
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
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .ok()
            .or_else(|| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ").ok())
    });

    // Auto-assign fallback if LLM didn't provide project/context
    let (fallback_project_ids, fallback_context_ids) =
        if project_ids.is_empty() && context_ids.is_empty() {
            auto_assign_context_fallback(
                ai,
                &parsed.description,
                projects,
                contexts,
                parsed.project_name.clone(),
                parsed.context_name.clone(),
            )
            .await
        } else {
            (vec![], vec![])
        };

    let final_project_ids = if project_ids.is_empty() && !fallback_project_ids.is_empty() {
        fallback_project_ids
    } else {
        project_ids
    };

    let final_context_ids = if context_ids.is_empty() && !fallback_context_ids.is_empty() {
        fallback_context_ids
    } else {
        context_ids
    };

    let task = Task {
        id: Uuid::now_v7(),
        completed: false,
        priority,
        description: parsed.description,
        project_ids: final_project_ids,
        context_ids: final_context_ids,
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

    tracing::info!(task_id = %created.id, source = %source_from_context(projects, contexts), "task created from LLM");
    Ok(created)
}

fn source_from_context(_projects: &[Project], _contexts: &[Context]) -> &'static str {
    "llm"
}

/// Embedding helper — used by the direct API route too
pub async fn generate_and_store_embedding(
    ai: &dyn AiProvider,
    task: &Task,
    db: &ParadeDbRepository,
) {
    let text = task_embedding_text(task);
    match ai.embed(&text).await {
        Ok(embedding) => {
            let _ = db.update_embedding(task.id, &embedding).await;
            tracing::debug!(task_id = %task.id, "embedding stored");
        }
        Err(e) => {
            tracing::warn!(task_id = %task.id, error = %e, "embedding generation failed");
        }
    }
}

fn task_embedding_text(task: &Task) -> String {
    let mut parts = vec![task.description.clone()];
    if let Some(p) = task.priority {
        parts.push(format!("({p})"));
    }
    parts.join(" ")
}
