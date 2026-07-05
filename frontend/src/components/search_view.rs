use dioxus::prelude::*;
use oxinbox_core::Task;

use crate::http;
use crate::storage;

fn status_class(status: &str) -> &'static str {
    match status {
        "inbox" => "status-inbox",
        "todo" => "status-todo",
        "doing" => "status-doing",
        "done" => "status-done",
        _ => "status-someday",
    }
}

#[component]
fn SearchResultCard(task: Task) -> Element {
    let cls = status_class(&format!("{:?}", task.status).to_lowercase());
    rsx! {
        div { class: "card {cls}",
            p {
                if let Some(p) = task.priority { span { "({p}) " } }
                "{task.description}"
            }
        }
    }
}

#[component]
pub fn SearchView() -> Element {
    let mut query = use_signal(String::new);
    let mut searching = use_signal(|| false);
    let mut results = use_signal(Vec::<Task>::new);
    let mut error = use_signal(|| None::<String>);

    let submit = move |_| {
        let q = query.read().trim().to_string();
        if q.is_empty() {
            return;
        }
        searching.set(true);
        error.set(None);
        results.set(Vec::new());

        spawn(async move {
            let payload = serde_json::json!({ "query": q, "limit": 20 });
            if let Some(token) = storage::get_token() {
                match http::api_post("/api/tasks/search", &payload, Some(&token)).await {
                    Ok(val) => {
                        if let Some(arr) = val["results"].as_array() {
                            let tasks: Vec<Task> = arr
                                .iter()
                                .filter_map(|r| serde_json::from_value(r["task"].clone()).ok())
                                .collect();
                            results.set(tasks);
                        }
                        searching.set(false);
                    }
                    Err(e) => {
                        error.set(Some(e));
                        searching.set(false);
                    }
                }
            }
        });
    };

    rsx! {
        div { class: "card",
            h3 { "Buscar tareas" }
            form {
                onsubmit: submit,
                div { class: "flex gap-2",
                    input {
                        class: "input flex-1",
                        placeholder: "Buscar por descripción, proyecto, contexto...",
                        value: query(),
                        oninput: move |e| query.set(e.value()),
                    }
                    button { r#type: "submit", disabled: searching(),
                        if searching() { "Buscando..." } else { "Buscar" }
                    }
                }
            }
            if let Some(msg) = error() {
                p { class: "text-sm", style: "color: var(--danger)", "{msg}" }
            }
            if !results.read().is_empty() {
                div { class: "mt-2",
                    p { class: "text-sm text-muted", "{results.read().len()} resultados" }
                    for task in results.read().iter() {
                        SearchResultCard { task: task.clone() }
                    }
                }
            }
        }
    }
}
