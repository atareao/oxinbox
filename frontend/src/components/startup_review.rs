use dioxus::prelude::*;
use oxinbox_core::Task;

use crate::http;
use crate::storage;

#[component]
pub fn StartupReview() -> Element {
    let mut stale_count = use_signal(|| 0usize);

    use_effect(move || {
        spawn(async move {
            if let Some(token) = storage::get_token()
                && let Ok(val) = http::api_get("/api/tasks", &token).await
                && let Ok(tasks) = serde_json::from_value::<Vec<Task>>(val)
            {
                let now = chrono::Utc::now();
                let count = tasks
                    .iter()
                    .filter(|t| {
                        t.status == oxinbox_core::TaskStatus::Inbox
                            && (now - t.created_at).num_hours() > 24
                    })
                    .count();
                stale_count.set(count);
            }
        });
    });

    let count = *stale_count.read();
    if count == 0 {
        return rsx! {};
    }

    let label = if count == 1 {
        "Tienes 1 tarea estancada en Inbox (+24h sin clasificar)".to_string()
    } else {
        format!("Tienes {count} tareas estancadas en Inbox (+24h sin clasificar)")
    };

    rsx! {
        div { class: "card", style: "border-left: 3px solid var(--warning, #f59e0b)",
            p { class: "text-sm", "{label}" }
        }
    }
}
