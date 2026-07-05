#![allow(clippy::future_not_send)]
#![allow(clippy::derive_partial_eq_without_eq)]
mod components;
mod db;
mod http;
mod storage;
mod sync;

use dioxus::prelude::*;

use crate::components::TaskDetail;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    use_context_provider(|| Signal::new(storage::get_token()));

    rsx! {
        document::Link { rel: "stylesheet", href: asset!("/public/styles.css") }
        Router::<Route> {}
    }
}

#[derive(Debug, Clone, PartialEq, Routable)]
enum Route {
    #[route("/")]
    Home {},
    #[route("/kanban")]
    Kanban {},
    #[route("/calendar")]
    Calendar {},
    #[route("/chat")]
    Chat {},
    #[route("/login")]
    Login {},
    #[route("/task/{id}")]
    TaskDetail { id: String },
}

#[component]
fn Home() -> Element {
    let token = use_context::<Signal<Option<String>>>();
    let mut tasks = use_signal(Vec::<oxinbox_core::Task>::new);

    if token.read().is_none() {
        return rsx! {
            div { class: "container", style: "padding-top: 40vh",
                div { class: "flex flex-col items-center gap-3",
                    h1 { "oxinbox" }
                    p { class: "text-muted", "Captura instantánea de tareas por voz" }
                    Link { to: Route::Login {}, "Iniciar sesión" }
                }
            }
        };
    }

    rsx! {
        div { class: "container",
            header { class: "flex justify-between items-center mb-3",
                h2 { "oxinbox" }
                nav { class: "flex gap-2",
                    Link { to: Route::Home {}, "Lista" }
                    Link { to: Route::Kanban {}, "Kanban" }
                    Link { to: Route::Calendar {}, "Calendario" }
                    Link { to: Route::Chat {}, "Chat" }
                }
            }
            components::StartupReview {}
            components::VoiceInput {
                on_task: move |task| {
                    let mut t = tasks.write();
                    t.push(task);
                },
            }
            components::TaskForm {
                on_created: move |task| {
                    let mut t = tasks.write();
                    t.push(task);
                },
            }
            components::PushSubscribe {}
            components::QueryView {}
            components::SearchView {}
            components::TaskList {}
        }
    }
}

#[component]
fn Kanban() -> Element {
    let token = use_context::<Signal<Option<String>>>();

    if token.read().is_none() {
        return rsx! {
            div { class: "container", style: "padding-top: 40vh",
                div { class: "flex flex-col items-center gap-3",
                    h1 { "oxinbox" }
                    Link { to: Route::Login {}, "Iniciar sesión" }
                }
            }
        };
    }

    rsx! {
        div { class: "container",
            header { class: "flex justify-between items-center mb-3",
                h2 { "oxinbox Kanban" }
                nav { class: "flex gap-2",
                    Link { to: Route::Home {}, "Lista" }
                    Link { to: Route::Kanban {}, "Kanban" }
                    Link { to: Route::Calendar {}, "Calendario" }
                    Link { to: Route::Chat {}, "Chat" }
                }
            }
            components::KanbanView {}
        }
    }
}

#[component]
fn Calendar() -> Element {
    let token = use_context::<Signal<Option<String>>>();

    if token.read().is_none() {
        return rsx! {
            div { class: "container", style: "padding-top: 40vh",
                div { class: "flex flex-col items-center gap-3",
                    h1 { "oxinbox" }
                    Link { to: Route::Login {}, "Iniciar sesión" }
                }
            }
        };
    }

    rsx! { components::CalendarView {} }
}

#[component]
fn Chat() -> Element {
    let token = use_context::<Signal<Option<String>>>();

    if token.read().is_none() {
        return rsx! {
            div { class: "container", style: "padding-top: 40vh",
                div { class: "flex flex-col items-center gap-3",
                    h1 { "oxinbox" }
                    Link { to: Route::Login {}, "Iniciar sesión" }
                }
            }
        };
    }

    rsx! { components::ChatView {} }
}

#[component]
fn Login() -> Element {
    let mut token = use_context::<Signal<Option<String>>>();

    if token.read().is_some() {
        return rsx! {
            div { class: "container", style: "padding-top: 40vh",
                div { class: "flex flex-col items-center gap-3",
                    h1 { "oxinbox" }
                    Link { to: Route::Home {}, "Ir a tareas" }
                }
            }
        };
    }

    rsx! {
        div { class: "container", style: "padding-top: 40vh",
            div { class: "flex flex-col items-center gap-3",
                h1 { "oxinbox" }
                p { class: "text-muted", "Captura instantánea de tareas por voz" }
                components::LoginButton {
                    on_login: move |t: String| {
                        storage::set_token(&t);
                        *token.write() = Some(t);
                    }
                }
            }
        }
    }
}
