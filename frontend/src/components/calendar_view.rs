use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use dioxus::prelude::*;
use oxinbox_core::Task;

use crate::http;
use crate::storage;

const fn month_name(m: u32) -> &'static str {
    match m {
        1 => "Enero",
        2 => "Febrero",
        3 => "Marzo",
        4 => "Abril",
        5 => "Mayo",
        6 => "Junio",
        7 => "Julio",
        8 => "Agosto",
        9 => "Septiembre",
        10 => "Octubre",
        11 => "Noviembre",
        12 => "Diciembre",
        _ => "",
    }
}

#[component]
fn CalendarDay(day: u32, tasks: Vec<Task>) -> Element {
    rsx! {
        div { class: "cal-day",
            div { class: "cal-day-num", "{day}" }
            for t in &tasks {
                Link {
                    to: crate::Route::TaskDetail { id: t.id.to_string() },
                    class: "cal-task",
                    span {
                        if let Some(p) = t.priority { span { "({p}) " } }
                        "{t.description}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn CalendarView() -> Element {
    let mut tasks_map = use_signal(HashMap::<NaiveDate, Vec<Task>>::new);
    let mut loading = use_signal(|| true);
    let mut current_month = use_signal(|| {
        let now = chrono::Utc::now();
        (now.month(), now.year())
    });

    use_effect(move || {
        spawn(async move {
            if let Some(token) = storage::get_token()
                && let Ok(val) = http::api_get("/api/tasks", &token).await
                && let Ok(all_tasks) = serde_json::from_value::<Vec<Task>>(val)
            {
                let mut map: HashMap<NaiveDate, Vec<Task>> = HashMap::new();
                for t in all_tasks {
                    if let Some(d) = t.due_date {
                        map.entry(d).or_default().push(t);
                    }
                }
                tasks_map.set(map);
            }
            loading.set(false);
        });
    });

    let (month, year) = *current_month.read();
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let last = {
        let next = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1)
        };
        next.unwrap().pred_opt().unwrap()
    };
    let start_weekday = first.weekday().num_days_from_monday() as usize;
    let total_days = last.day() as usize;

    let mut weeks: Vec<Vec<Option<u32>>> = Vec::new();
    let mut week = vec![None; 7];
    for d in 0..total_days {
        let wd = (start_weekday + d) % 7;
        if wd == 0 && d > 0 {
            weeks.push(week);
            week = vec![None; 7];
        }
        week[wd] = Some(u32::try_from(d + 1).unwrap());
    }
    weeks.push(week);

    let prev = move |_| {
        let (m, y) = *current_month.read();
        if m == 1 {
            current_month.set((12, y - 1));
        } else {
            current_month.set((m - 1, y));
        }
    };

    let next = move |_| {
        let (m, y) = *current_month.read();
        if m == 12 {
            current_month.set((1, y + 1));
        } else {
            current_month.set((m + 1, y));
        }
    };

    #[allow(clippy::type_complexity)]
    let week_data: Vec<Vec<Option<(u32, Vec<Task>)>>> = weeks
        .iter()
        .map(|w| {
            w.iter()
                .map(|d| {
                    d.map(|day| {
                        let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
                        let day_tasks = tasks_map.read().get(&date).cloned().unwrap_or_default();
                        (day, day_tasks)
                    })
                })
                .collect()
        })
        .collect();

    rsx! {
        div { class: "container",
            header { class: "flex justify-between items-center mb-3",
                nav { class: "flex gap-2",
                    Link { to: crate::Route::Home {}, "Lista" }
                    Link { to: crate::Route::Kanban {}, "Kanban" }
                    Link { to: crate::Route::Calendar {}, "Calendario" }
                }
            }
            div { class: "card",
                div { class: "flex justify-between items-center mb-2",
                    button { onclick: prev, "\u{2190}" }
                    h3 { "{month_name(month)} {year}" }
                    button { onclick: next, "\u{2192}" }
                }
                if loading() {
                    p { class: "text-muted", "Cargando..." }
                } else {
                    div { class: "cal-grid",
                        div { class: "cal-header", "Lun" }
                        div { class: "cal-header", "Mar" }
                        div { class: "cal-header", "Mi\u{e9}" }
                        div { class: "cal-header", "Jue" }
                        div { class: "cal-header", "Vie" }
                        div { class: "cal-header", "S\u{e1}b" }
                        div { class: "cal-header", "Dom" }
                        for w in &week_data {
                            for item in w {
                                if let Some((day, day_tasks)) = item {
                                    CalendarDay { day: *day, tasks: day_tasks.clone() }
                                } else {
                                    div { class: "cal-day cal-day-empty" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
