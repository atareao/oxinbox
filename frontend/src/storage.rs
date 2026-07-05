use oxinbox_core::Task;
use web_sys::window;

const TOKEN_KEY: &str = "oxinbox_token";
const TASKS_KEY: &str = "oxinbox_tasks";

fn storage() -> Option<web_sys::Storage> {
    window()?.local_storage().ok()?
}

pub fn get_token() -> Option<String> {
    storage()?.get_item(TOKEN_KEY).ok()?
}

pub fn set_token(token: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(TOKEN_KEY, token);
    }
}

#[expect(dead_code)]
pub fn clear_token() {
    if let Some(s) = storage() {
        let _ = s.remove_item(TOKEN_KEY);
    }
}

pub fn save_tasks(tasks: &[Task]) {
    if let Ok(json) = serde_json::to_string(tasks)
        && let Some(s) = storage()
    {
        let _ = s.set_item(TASKS_KEY, &json);
    }
}

pub fn load_tasks() -> Vec<Task> {
    storage()
        .and_then(|s| s.get_item(TASKS_KEY).ok()?)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save_task(task: &Task) {
    let mut tasks = load_tasks();
    if let Some(pos) = tasks.iter().position(|t| t.id == task.id) {
        tasks[pos] = task.clone();
    } else {
        tasks.push(task.clone());
    }
    save_tasks(&tasks);
}

pub fn delete_task(id: &uuid::Uuid) {
    let mut tasks = load_tasks();
    tasks.retain(|t| t.id != *id);
    save_tasks(&tasks);
}
