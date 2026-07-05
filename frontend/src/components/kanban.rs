use dioxus::prelude::*;
use oxinbox_core::{Task, TaskStatus};

use crate::http;
use crate::storage;

const COLUMNS: &[TaskStatus] = &[
    TaskStatus::Inbox,
    TaskStatus::Todo,
    TaskStatus::Doing,
    TaskStatus::Done,
    TaskStatus::Someday,
];

const fn status_label(s: &TaskStatus) -> &'static str {
    match s {
        TaskStatus::Inbox => "Inbox",
        TaskStatus::Todo => "Todo",
        TaskStatus::Doing => "Doing",
        TaskStatus::Done => "Done",
        TaskStatus::Someday => "Someday",
    }
}

const fn status_class(s: &TaskStatus) -> &'static str {
    match s {
        TaskStatus::Inbox => "kanban-col-inbox",
        TaskStatus::Todo => "kanban-col-todo",
        TaskStatus::Doing => "kanban-col-doing",
        TaskStatus::Done => "kanban-col-done",
        TaskStatus::Someday => "kanban-col-someday",
    }
}

#[component]
fn KanbanCard(task: Task) -> Element {
    rsx! {
        Link {
            to: crate::Route::TaskDetail { id: task.id.to_string() },
            class: "kanban-card",
            div { class: "flex flex-col gap-1",
                p { class: "text-sm",
                    if let Some(p) = task.priority { span { "({p}) " } }
                    "{task.description}"
                }
                div { class: "flex gap-1 text-xs text-muted",
                    if !task.projects.is_empty() {
                        for p in &task.projects { span { "+{p} " } }
                    }
                    if !task.contexts.is_empty() {
                        for c in &task.contexts { span { "@{c} " } }
                    }
                }
            }
        }
    }
}

#[component]
fn KanbanColumn(status: TaskStatus, tasks: Vec<Task>) -> Element {
    let label = status_label(&status);
    let cls = status_class(&status);
    let count = tasks.len();

    rsx! {
        div { class: "kanban-col {cls}",
            div { class: "kanban-col-header",
                h3 { "{label}" }
                span { class: "kanban-count", "{count}" }
            }
            div { class: "kanban-col-body",
                if tasks.is_empty() {
                    p { class: "text-muted text-sm", "Sin tareas" }
                }
                for task in tasks {
                    KanbanCard { task }
                }
            }
        }
    }
}

#[component]
pub fn KanbanView() -> Element {
    let mut tasks_by_status = use_signal(|| {
        let m: std::collections::HashMap<String, Vec<Task>> = COLUMNS
            .iter()
            .map(|s| (format!("{s:?}").to_lowercase(), Vec::new()))
            .collect();
        m
    });
    let mut loading = use_signal(|| true);

    use_effect(move || {
        spawn(async move {
            if let Some(token) = storage::get_token()
                && let Ok(val) = http::api_get("/api/tasks", &token).await
                && let Ok(all_tasks) = serde_json::from_value::<Vec<Task>>(val)
            {
                let mut map: std::collections::HashMap<String, Vec<Task>> = COLUMNS
                    .iter()
                    .map(|s| (format!("{s:?}").to_lowercase(), Vec::new()))
                    .collect();
                for task in all_tasks {
                    let key = format!("{:?}", task.status).to_lowercase();
                    map.entry(key).or_default().push(task);
                }
                tasks_by_status.set(map);
            }
            loading.set(false);
        });
    });

    let cols: Vec<(TaskStatus, Vec<Task>)> = if loading() {
        Vec::new()
    } else {
        let map = tasks_by_status.read();
        COLUMNS
            .iter()
            .map(|s| {
                let key = format!("{s:?}").to_lowercase();
                let tasks = map.get(&key).cloned().unwrap_or_default();
                (s.clone(), tasks)
            })
            .collect()
    };

    rsx! {
        div { class: "kanban-board",
            if loading() {
                p { class: "text-muted", "Cargando..." }
            } else {
                for (col_status, col_tasks) in cols {
                    KanbanColumn {
                        status: col_status,
                        tasks: col_tasks,
                    }
                }
            }
        }
    }
}
