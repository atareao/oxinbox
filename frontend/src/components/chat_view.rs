use dioxus::prelude::*;

use crate::http;
use crate::storage;

#[derive(Debug, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

#[component]
pub fn ChatView() -> Element {
    let mut messages = use_signal(Vec::<ChatMessage>::new);
    let mut input = use_signal(String::new);
    let mut sending = use_signal(|| false);

    let send = move |_| {
        let text = input.read().trim().to_string();
        if text.is_empty() {
            return;
        }
        input.set(String::new());
        messages.write().push(ChatMessage {
            role: "user".into(),
            content: text.clone(),
        });
        sending.set(true);

        spawn(async move {
            let payload = serde_json::json!({ "query": text });
            if let Some(token) = storage::get_token() {
                match http::api_post("/api/query", &payload, Some(&token)).await {
                    Ok(val) => {
                        let answer = val["answer"].as_str().unwrap_or("Sin respuesta");
                        messages.write().push(ChatMessage {
                            role: "assistant".into(),
                            content: answer.to_string(),
                        });
                    }
                    Err(e) => {
                        messages.write().push(ChatMessage {
                            role: "assistant".into(),
                            content: format!("Error: {e}"),
                        });
                    }
                }
            }
            sending.set(false);
        });
    };

    let msgs: Vec<(String, String, String)> = messages
        .read()
        .iter()
        .map(|m| {
            let cls = if m.role == "user" {
                "chat-msg-user"
            } else {
                "chat-msg-assistant"
            };
            (m.role.clone(), m.content.clone(), cls.to_string())
        })
        .collect();

    rsx! {
        div { class: "container",
            header { class: "flex justify-between items-center mb-3",
                nav { class: "flex gap-2",
                    Link { to: crate::Route::Home {}, "Lista" }
                    Link { to: crate::Route::Kanban {}, "Kanban" }
                    Link { to: crate::Route::Calendar {}, "Calendario" }
                    Link { to: crate::Route::Chat {}, "Chat" }
                }
            }
            div { class: "card",
                h3 { "Chat GTD" }
                p { class: "text-sm text-muted", "Pregunta sobre tus tareas en lenguaje natural" }
            }
            div { class: "chat-messages",
                for (_, content, cls) in &msgs {
                    div { class: "chat-msg {cls}",
                        p { "{content}" }
                    }
                }
                if sending() {
                    div { class: "chat-msg chat-msg-assistant",
                        p { class: "text-muted", "Pensando..." }
                    }
                }
            }
            form {
                class: "chat-input",
                onsubmit: send,
                div { class: "flex gap-2",
                    input {
                        class: "input flex-1",
                        placeholder: "Ej: qué tareas vencen esta semana",
                        value: input(),
                        oninput: move |e| input.set(e.value()),
                    }
                    button { r#type: "submit", disabled: sending(),
                        if sending() { "..." } else { "Enviar" }
                    }
                }
            }
        }
    }
}
