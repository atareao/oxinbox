use dioxus::prelude::*;
use oxinbox_core::Task;
use uuid::Uuid;

use crate::Route;
use crate::http;
use crate::storage;
use crate::sync;

#[component]
pub fn TaskDetail(id: String) -> Element {
    let token = use_context::<Signal<Option<String>>>();
    let navigator = use_navigator();

    let Ok(task_id) = Uuid::parse_str(&id) else {
        return rsx! { p { "ID inválido" } };
    };

    let mut task = use_signal(|| None::<Task>);
    let mut loading = use_signal(|| true);
    let mut edit_desc = use_signal(String::new);
    let mut edit_priority = use_signal(|| None::<char>);
    let mut edit_projects = use_signal(String::new);
    let mut edit_contexts = use_signal(String::new);
    let mut edit_due = use_signal(String::new);
    let mut edit_status = use_signal(String::new);
    let mut saving = use_signal(|| false);
    let mut deleting = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    use_effect(move || {
        spawn(async move {
            if let Some(t) = &token()
                && let Ok(val) = http::api_get(&format!("/api/tasks/{task_id}"), t).await
                && let Ok(t) = serde_json::from_value::<Task>(val)
            {
                edit_desc.set(t.description.clone());
                edit_priority.set(t.priority);
                edit_projects.set(t.projects.join(", "));
                edit_contexts.set(t.contexts.join(", "));
                edit_due.set(t.due_date.map(|d| d.to_string()).unwrap_or_default());
                edit_status.set(format!("{:?}", t.status).to_lowercase());
                task.set(Some(t));
            }
            loading.set(false);
        });
    });

    let save = move |_| {
        saving.set(true);
        error.set(None);
        let desc = edit_desc.read().trim().to_string();
        let prio = *edit_priority.read();
        let proj: Vec<String> = edit_projects
            .read()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let ctx: Vec<String> = edit_contexts
            .read()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let due = edit_due.read().clone();
        let due_val = if due.is_empty() {
            None
        } else {
            chrono::NaiveDate::parse_from_str(&due, "%Y-%m-%d").ok()
        };
        let status = edit_status.read().clone();
        let tid = task_id;
        let nav = navigator;
        let tk = token.read().clone();

        spawn(async move {
            if let Some(t) = tk {
                let status_str = if status == "inbox"
                    || status == "todo"
                    || status == "doing"
                    || status == "done"
                    || status == "someday"
                {
                    Some(status)
                } else {
                    None
                };
                match sync::update_remote_task(&t, tid, desc, prio, proj, ctx, due_val, status_str)
                    .await
                {
                    Ok(task) => {
                        storage::save_task(&task);
                        nav.push(Route::Home {});
                    }
                    Err(e) => {
                        saving.set(false);
                        error.set(Some(e));
                    }
                }
            }
        });
    };

    let delete = move |_| {
        deleting.set(true);
        let tid = task_id;
        let nav = navigator;
        let tk = token.read().clone();

        spawn(async move {
            if let Some(t) = tk {
                match sync::delete_remote_task(&t, tid).await {
                    Ok(()) => {
                        nav.push(Route::Home {});
                    }
                    Err(e) => {
                        deleting.set(false);
                        error.set(Some(e));
                    }
                }
            }
        });
    };

    if token.read().is_none() {
        return rsx! {
            div { class: "container", style: "padding-top: 40vh",
                Link { to: Route::Login {}, "Iniciar sesión" }
            }
        };
    }

    if loading() {
        return rsx! { div { class: "container", p { "Cargando..." } } };
    }

    let Some(_t) = task.read().clone() else {
        return rsx! {
            div { class: "container",
                p { "Tarea no encontrada" }
                Link { to: Route::Home {}, "Volver" }
            }
        };
    };

    rsx! {
        div { class: "container",
            header { class: "flex justify-between items-center mb-3",
                h2 { "Editar tarea" }
                Link { to: Route::Home {}, "← Volver" }
            }
            form {
                class: "card",
                onsubmit: save,
                div { class: "flex flex-col gap-2",
                    label { "Descripción" }
                    input {
                        class: "input",
                        value: edit_desc(),
                        oninput: move |e| edit_desc.set(e.value()),
                    }
                    div { class: "flex gap-2",
                        div { class: "flex-1",
                            label { "Prioridad" }
                            select {
                                class: "input",
                                value: edit_priority().map(|c| c.to_string()).unwrap_or_default(),
                                onchange: move |e| {
                                    let v = e.value();
                                    edit_priority.set(if v.is_empty() { None } else { v.chars().next() });
                                },
                                option { value: "", "-- Ninguna --" }
                                option { value: "A", "A - Alta" }
                                option { value: "B", "B - Media" }
                                option { value: "C", "C - Baja" }
                            }
                        }
                        div { class: "flex-1",
                            label { "Estado" }
                            select {
                                class: "input",
                                value: edit_status(),
                                onchange: move |e| edit_status.set(e.value()),
                                option { value: "inbox", "Inbox" }
                                option { value: "todo", "Todo" }
                                option { value: "doing", "Doing" }
                                option { value: "done", "Done" }
                                option { value: "someday", "Someday" }
                            }
                        }
                        div { class: "flex-1",
                            label { "Vence" }
                            input {
                                class: "input",
                                r#type: "date",
                                value: edit_due(),
                                oninput: move |e| edit_due.set(e.value()),
                            }
                        }
                    }
                    div { class: "flex gap-2",
                        div { class: "flex-1",
                            label { "Proyectos" }
                            input {
                                class: "input",
                                placeholder: "proyecto1, proyecto2",
                                value: edit_projects(),
                                oninput: move |e| edit_projects.set(e.value()),
                            }
                        }
                        div { class: "flex-1",
                            label { "Contextos" }
                            input {
                                class: "input",
                                placeholder: "casa, trabajo",
                                value: edit_contexts(),
                                oninput: move |e| edit_contexts.set(e.value()),
                            }
                        }
                    }
                    if let Some(msg) = error() {
                        p { class: "text-sm", style: "color: var(--danger)", "{msg}" }
                    }
                    div { class: "flex gap-2 justify-between",
                        button { r#type: "submit", disabled: saving(),
                            if saving() { "Guardando..." } else { "Guardar cambios" }
                        }
                        button {
                            onclick: delete,
                            disabled: deleting(),
                            style: "background: var(--danger, #e74c3c)",
                            if deleting() { "Eliminando..." } else { "Eliminar tarea" }
                        }
                    }
                }
            }
        }
    }
}
