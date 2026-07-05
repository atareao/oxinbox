use dioxus::prelude::*;
use oxinbox_core::Task;

use crate::storage;
use crate::sync;

#[component]
pub fn TaskForm(on_created: EventHandler<Task>) -> Element {
    let mut description = use_signal(String::new);
    let mut priority = use_signal(|| None::<char>);
    let mut projects = use_signal(String::new);
    let mut contexts = use_signal(String::new);
    let mut due_date = use_signal(String::new);
    let mut submitting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let can_submit = !description.read().trim().is_empty();

    let submit = move |_| {
        if !can_submit {
            return;
        }
        submitting.set(true);
        error.set(None);

        let desc = description.read().trim().to_string();
        let prio = *priority.read();
        let proj: Vec<String> = projects
            .read()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let ctx: Vec<String> = contexts
            .read()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let due = due_date.read().clone();
        let due_val = if due.is_empty() {
            None
        } else {
            chrono::NaiveDate::parse_from_str(&due, "%Y-%m-%d").ok()
        };

        spawn(async move {
            let token = storage::get_token();
            let Some(token) = token.as_deref() else {
                error.set(Some("no auth".into()));
                submitting.set(false);
                return;
            };

            match sync::create_task(desc, prio, proj, ctx, due_val, token).await {
                Ok(task) => {
                    on_created.call(task);
                    description.set(String::new());
                    priority.set(None);
                    projects.set(String::new());
                    contexts.set(String::new());
                    due_date.set(String::new());
                    submitting.set(false);
                }
                Err(e) => {
                    error.set(Some(e));
                    submitting.set(false);
                }
            }
        });
    };

    rsx! {
        form {
            class: "card",
            onsubmit: submit,
            div { class: "flex flex-col gap-2",
                input {
                    class: "input",
                    placeholder: "Descripción de la tarea...",
                    value: description(),
                    oninput: move |e| description.set(e.value()),
                }
                div { class: "flex gap-2",
                    select {
                        class: "input",
                        value: priority().map(|c| c.to_string()).unwrap_or_default(),
                        onchange: move |e| {
                            let v = e.value();
                            priority.set(if v.is_empty() { None } else { v.chars().next() });
                        },
                        option { value: "", "-- Prioridad --" }
                        option { value: "A", "A - Alta" }
                        option { value: "B", "B - Media" }
                        option { value: "C", "C - Baja" }
                    }
                    input {
                        class: "input",
                        placeholder: "Proyectos (+proy)",
                        value: projects(),
                        oninput: move |e| projects.set(e.value()),
                    }
                    input {
                        class: "input",
                        placeholder: "Contextos (@lugar)",
                        value: contexts(),
                        oninput: move |e| contexts.set(e.value()),
                    }
                }
                div { class: "flex gap-2",
                    input {
                        class: "input",
                        r#type: "date",
                        value: due_date(),
                        oninput: move |e| due_date.set(e.value()),
                    }
                }
                if let Some(msg) = error() {
                    p { class: "text-sm", style: "color: var(--danger)", "{msg}" }
                }
                button {
                    disabled: !can_submit || submitting(),
                    if submitting() { "Creando..." } else { "Crear tarea" }
                }
            }
        }
    }
}
