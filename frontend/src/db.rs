#![allow(dead_code)]
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(inline_js = r#"
export function openDb(name, version) {
    return new Promise((resolve, reject) => {
        const req = indexedDB.open(name, version);
        req.onupgradeneeded = (e) => {
            const db = e.target.result;
            if (!db.objectStoreNames.contains('tasks')) {
                const store = db.createObjectStore('tasks', { keyPath: 'id' });
                store.createIndex('updated_at', 'updated_at', { unique: false });
            }
            if (!db.objectStoreNames.contains('pending_ops')) {
                db.createObjectStore('pending_ops', { keyPath: 'id', autoIncrement: true });
            }
        };
        req.onsuccess = (e) => resolve(e.target.result);
        req.onerror = () => reject(req.error);
    });
}

export function saveTask(db, task) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('tasks', 'readwrite');
        const store = tx.objectStore('tasks');
        const req = store.put(task);
        req.onsuccess = () => resolve();
        req.onerror = () => reject(req.error);
    });
}

export function saveTasks(db, tasks) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('tasks', 'readwrite');
        const store = tx.objectStore('tasks');
        let n = 0;
        for (const t of tasks) {
            const req = store.put(t);
            req.onsuccess = () => { n++; if (n === tasks.length) resolve(); };
            req.onerror = () => reject(req.error);
        }
        if (tasks.length === 0) resolve();
    });
}

export function listTasks(db) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('tasks', 'readonly');
        const store = tx.objectStore('tasks');
        const req = store.getAll();
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
}

export function deleteTask(db, id) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('tasks', 'readwrite');
        const store = tx.objectStore('tasks');
        const req = store.delete(id);
        req.onsuccess = () => resolve();
        req.onerror = () => reject(req.error);
    });
}

export function clearTasks(db) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('tasks', 'readwrite');
        const store = tx.objectStore('tasks');
        const req = store.clear();
        req.onsuccess = () => resolve();
        req.onerror = () => reject(req.error);
    });
}

export function addPendingOp(db, op) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('pending_ops', 'readwrite');
        const store = tx.objectStore('pending_ops');
        const req = store.add(op);
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
}

export function listPendingOps(db) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('pending_ops', 'readonly');
        const store = tx.objectStore('pending_ops');
        const req = store.getAll();
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
    });
}

export function deletePendingOp(db, id) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('pending_ops', 'readwrite');
        const store = tx.objectStore('pending_ops');
        const req = store.delete(id);
        req.onsuccess = () => resolve();
        req.onerror = () => reject(req.error);
    });
}

export function clearPendingOps(db) {
    return new Promise((resolve, reject) => {
        const tx = db.transaction('pending_ops', 'readwrite');
        const store = tx.objectStore('pending_ops');
        const req = store.clear();
        req.onsuccess = () => resolve();
        req.onerror = () => reject(req.error);
    });
}
"#)]
extern "C" {
    pub type JsDb;

    fn openDb(name: String, version: i32) -> js_sys::Promise;
    fn saveTask(db: &JsDb, task: JsValue) -> js_sys::Promise;
    fn saveTasks(db: &JsDb, tasks: JsValue) -> js_sys::Promise;
    fn listTasks(db: &JsDb) -> js_sys::Promise;
    fn deleteTask(db: &JsDb, id: String) -> js_sys::Promise;
    fn clearTasks(db: &JsDb) -> js_sys::Promise;

    fn addPendingOp(db: &JsDb, op: JsValue) -> js_sys::Promise;
    fn listPendingOps(db: &JsDb) -> js_sys::Promise;
    fn deletePendingOp(db: &JsDb, id: f64) -> js_sys::Promise;
    fn clearPendingOps(db: &JsDb) -> js_sys::Promise;
}

use oxinbox_core::Task;

pub struct Db {
    pub inner: JsDb,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PendingOp {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<f64>,
    pub url: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    pub token: String,
    pub created_at: i64,
}

impl Db {
    pub async fn open() -> Result<Self, JsValue> {
        let val = JsFuture::from(openDb("oxinbox".into(), 2)).await?;
        Ok(Self { inner: val.into() })
    }

    pub async fn save_task(&self, task: &Task) -> Result<(), JsValue> {
        let val = serde_wasm_bindgen::to_value(task)?;
        JsFuture::from(saveTask(&self.inner, val)).await?;
        Ok(())
    }

    pub async fn save_tasks(&self, tasks: &[Task]) -> Result<(), JsValue> {
        let arr = js_sys::Array::new();
        for t in tasks {
            arr.push(&serde_wasm_bindgen::to_value(t)?);
        }
        JsFuture::from(saveTasks(&self.inner, arr.into())).await?;
        Ok(())
    }

    pub async fn list_tasks(&self) -> Result<Vec<Task>, JsValue> {
        let val = JsFuture::from(listTasks(&self.inner)).await?;
        if val.is_undefined() || val.is_null() {
            return Ok(Vec::new());
        }
        let arr: js_sys::Array = val.into();
        let mut tasks = Vec::with_capacity(arr.length() as usize);
        for item in arr.iter() {
            if let Ok(t) = serde_wasm_bindgen::from_value(item) {
                tasks.push(t);
            }
        }
        Ok(tasks)
    }

    pub async fn delete_task(&self, id: &uuid::Uuid) -> Result<(), JsValue> {
        JsFuture::from(deleteTask(&self.inner, id.to_string())).await?;
        Ok(())
    }

    pub async fn clear_tasks(&self) -> Result<(), JsValue> {
        JsFuture::from(clearTasks(&self.inner)).await?;
        Ok(())
    }

    pub async fn add_pending_op(&self, op: &PendingOp) -> Result<f64, JsValue> {
        let val = serde_wasm_bindgen::to_value(op)?;
        let id = JsFuture::from(addPendingOp(&self.inner, val)).await?;
        Ok(id.as_f64().unwrap_or(0.0))
    }

    pub async fn list_pending_ops(&self) -> Result<Vec<PendingOp>, JsValue> {
        let val = JsFuture::from(listPendingOps(&self.inner)).await?;
        if val.is_undefined() || val.is_null() {
            return Ok(Vec::new());
        }
        let arr: js_sys::Array = val.into();
        let mut ops = Vec::with_capacity(arr.length() as usize);
        for item in arr.iter() {
            if let Ok(op) = serde_wasm_bindgen::from_value(item) {
                ops.push(op);
            }
        }
        Ok(ops)
    }

    pub async fn delete_pending_op(&self, id: f64) -> Result<(), JsValue> {
        JsFuture::from(deletePendingOp(&self.inner, id)).await?;
        Ok(())
    }

    pub async fn clear_pending_ops(&self) -> Result<(), JsValue> {
        JsFuture::from(clearPendingOps(&self.inner)).await?;
        Ok(())
    }
}
