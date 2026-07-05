use oxinbox_core::Task;
use uuid::Uuid;

use crate::db::{Db, PendingOp};
use crate::http;
use crate::storage;

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function registerBgSync() {
    if (!navigator.serviceWorker || !('sync' in ServiceWorkerRegistration.prototype)) {
        return Promise.resolve(false);
    }
    return navigator.serviceWorker.ready.then(function(reg) {
        return reg.sync.register('sync-tasks').then(function() {
            return true;
        }).catch(function() {
            return false;
        });
    }).catch(function() {
        return false;
    });
}
"#)]
extern "C" {
    fn registerBgSync() -> js_sys::Promise;
}

async fn register_bg_sync() {
    let _ = wasm_bindgen_futures::JsFuture::from(registerBgSync()).await;
}

async fn queue_op(url: String, method: String, body: Option<serde_json::Value>, token: String) {
    if let Ok(db) = Db::open().await {
        let op = PendingOp {
            id: None,
            url,
            method,
            body,
            token,
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        let _ = db.add_pending_op(&op).await;
    }
    register_bg_sync().await;
}

pub async fn create_task(
    description: String,
    priority: Option<char>,
    projects: Vec<String>,
    contexts: Vec<String>,
    due_date: Option<chrono::NaiveDate>,
    token: &str,
) -> Result<Task, String> {
    let payload = serde_json::json!({
        "description": description,
        "priority": priority,
        "projects": projects,
        "contexts": contexts,
        "due_date": due_date,
    });

    if let Ok(val) = http::api_post("/api/tasks", &payload, Some(token)).await {
        let t: Task = serde_json::from_value(val).map_err(|e| format!("json: {e}"))?;
        if let Ok(db) = Db::open().await {
            let _ = db.save_task(&t).await;
        }
        storage::save_task(&t);
        Ok(t)
    } else {
        queue_op(
            "/api/tasks".into(),
            "POST".into(),
            Some(payload),
            token.to_string(),
        )
        .await;
        Err("offline".into())
    }
}

#[expect(clippy::too_many_arguments)]
pub async fn update_remote_task(
    token: &str,
    id: Uuid,
    description: String,
    priority: Option<char>,
    projects: Vec<String>,
    contexts: Vec<String>,
    due_date: Option<chrono::NaiveDate>,
    status: Option<String>,
) -> Result<Task, String> {
    let mut payload = serde_json::json!({
        "description": description,
        "priority": priority,
        "projects": projects,
        "contexts": contexts,
        "due_date": due_date,
    });
    if let Some(s) = &status {
        payload["status"] = serde_json::Value::String(s.clone());
    }

    if let Ok(val) = http::api_put(&format!("/api/tasks/{id}"), &payload, token).await {
        let t: Task = serde_json::from_value(val).map_err(|e| format!("json: {e}"))?;
        if let Ok(db) = Db::open().await {
            let _ = db.save_task(&t).await;
        }
        storage::save_task(&t);
        Ok(t)
    } else {
        queue_op(
            format!("/api/tasks/{id}"),
            "PUT".into(),
            Some(payload),
            token.to_string(),
        )
        .await;
        Err("offline".into())
    }
}

pub async fn delete_remote_task(token: &str, id: Uuid) -> Result<(), String> {
    if http::api_delete(&format!("/api/tasks/{id}"), token).await == Ok(()) {
        if let Ok(db) = Db::open().await {
            let _ = db.delete_task(&id).await;
        }
        storage::delete_task(&id);
        Ok(())
    } else {
        queue_op(
            format!("/api/tasks/{id}"),
            "DELETE".into(),
            None,
            token.to_string(),
        )
        .await;
        Err("offline".into())
    }
}
