use dioxus::prelude::*;
use oxinbox_core::Task;

use crate::Route;
use crate::http;
use crate::storage;

#[component]
pub fn TaskList() -> Element {
    let mut tasks = use_signal(storage::load_tasks);
    let mut loading = use_signal(|| true);

    use_effect(move || {
        spawn(async move {
            if let Some(token) = storage::get_token()
                && let Ok(val) = http::api_get("/api/tasks", &token).await
                && let Ok(server_tasks) = serde_json::from_value::<Vec<Task>>(val)
            {
                storage::save_tasks(&server_tasks);
                tasks.set(server_tasks);
            } else {
                let cached = storage::load_tasks();
                if !cached.is_empty() {
                    tasks.set(cached);
                }
            }
            loading.set(false);
        });
    });

    rsx! {
        div { class: "container",
            header { class: "flex justify-between items-center mb-3",
                h2 { "Tareas" }
                button {
                    onclick: move |_| {
                        spawn(async move {
                            if let Some(token) = storage::get_token()
                                && let Ok(val) = http::api_get("/api/tasks", &token).await
                                && let Ok(t) = serde_json::from_value::<Vec<Task>>(val)
                            {
                                storage::save_tasks(&t);
                                tasks.set(t);
                            }
                        });
                    },
                    "Refrescar"
                }
            }
            if loading() {
                p { class: "text-muted", "Cargando..." }
            } else if tasks.read().is_empty() {
                p { class: "text-muted", "No hay tareas. Usa el formulario para crear una." }
            } else {
                for task in tasks.read().iter() {
                    TaskCard { task: task.clone() }
                }
            }
        }
    }
}

#[component]
fn TaskCard(task: Task) -> Element {
    let status_class = format!(
        "status-{}",
        match task.status {
            oxinbox_core::TaskStatus::Inbox => "inbox",
            oxinbox_core::TaskStatus::Todo => "todo",
            oxinbox_core::TaskStatus::Doing => "doing",
            oxinbox_core::TaskStatus::Done => "done",
            oxinbox_core::TaskStatus::Someday => "someday",
        }
    );

    rsx! {
        Link {
            to: Route::TaskDetail { id: task.id.to_string() },
            class: "card {status_class}",
            div { class: "flex justify-between items-center",
                div { class: "flex-col gap-2",
                    p {
                        if let Some(p) = task.priority {
                            span { "({p}) " }
                        }
                        "{task.description}"
                    }
                    div { class: "flex gap-2 text-sm text-muted",
                        if !task.projects.is_empty() {
                            for p in &task.projects {
                                span { "+{p} " }
                            }
                        }
                        if !task.contexts.is_empty() {
                            for c in &task.contexts {
                                span { "@{c} " }
                            }
                        }
                        if let Some(d) = task.due_date {
                            span { "📅 {d}" }
                        }
                    }
                }
            }
        }
    }
}
