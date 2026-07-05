use dioxus::prelude::*;
use serde_json::Value;

use crate::http;
use crate::storage;

#[component]
pub fn QueryView() -> Element {
    let mut query = use_signal(String::new);
    let mut querying = use_signal(|| false);
    let mut sql = use_signal(String::new);
    let mut answer = use_signal(String::new);
    let mut results = use_signal(Vec::<Value>::new);
    let mut error = use_signal(|| None::<String>);

    let submit = move |_| {
        let q = query.read().trim().to_string();
        if q.is_empty() {
            return;
        }
        querying.set(true);
        error.set(None);
        sql.set(String::new());
        answer.set(String::new());
        results.set(Vec::new());

        spawn(async move {
            let payload = serde_json::json!({ "query": q });
            if let Some(token) = storage::get_token() {
                match http::api_post("/api/query", &payload, Some(&token)).await {
                    Ok(val) => {
                        if let Some(s) = val["sql"].as_str() {
                            sql.set(s.to_string());
                        }
                        if let Some(a) = val["answer"].as_str() {
                            answer.set(a.to_string());
                        }
                        if let Some(r) = val["results"].as_array() {
                            results.set(r.clone());
                        }
                        querying.set(false);
                    }
                    Err(e) => {
                        error.set(Some(e));
                        querying.set(false);
                    }
                }
            }
        });
    };

    rsx! {
        div { class: "card",
            h3 { "Consulta en lenguaje natural" }
            form {
                onsubmit: submit,
                div { class: "flex gap-2",
                    input {
                        class: "input flex-1",
                        placeholder: "Ej: qué tareas tengo que hacer hoy",
                        value: query(),
                        oninput: move |e| query.set(e.value()),
                    }
                    button { r#type: "submit", disabled: querying(),
                        if querying() { "Consultando..." } else { "Preguntar" }
                    }
                }
            }
            if let Some(msg) = error() {
                p { class: "text-sm", style: "color: var(--danger)", "{msg}" }
            }
            if !sql.read().is_empty() {
                div { class: "mt-2",
                    p { class: "text-sm text-muted", "SQL generado:" }
                    pre { class: "sql-block", "{sql}" }
                }
            }
            if !answer.read().is_empty() {
                div { class: "mt-2",
                    p { class: "text-sm", "{answer}" }
                }
            }
            if !results.read().is_empty() {
                div { class: "mt-2",
                    p { class: "text-sm text-muted", "Resultados ({results.read().len()}):" }
                    for r in results.read().iter() {
                        TaskResultRow { task: r.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn TaskResultRow(task: Value) -> Element {
    let desc = task["description"].as_str().unwrap_or("");
    let status = task["status"].as_str().unwrap_or("");
    let prio = task["priority"].as_str().and_then(|s| s.chars().next());
    let projects: Vec<&str> = task["projects"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let contexts: Vec<&str> = task["contexts"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    rsx! {
        div { class: "card status-{status}",
            div { class: "flex justify-between",
                p {
                    if let Some(p) = prio { span { "({p}) " } }
                    "{desc}"
                }
                div { class: "flex gap-1 text-sm text-muted",
                    for p in &projects { span { "+{p} " } }
                    for c in &contexts { span { "@{c} " } }
                }
            }
        }
    }
}
