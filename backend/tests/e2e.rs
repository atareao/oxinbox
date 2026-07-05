#![allow(clippy::significant_drop_tightening)]

use std::collections::HashMap;
use std::sync::Arc;

use oxinbox_backend::auth::AuthState;
use oxinbox_backend::push::PushService;
use oxinbox_backend::repository::InMemoryTaskRepository;
use oxinbox_backend::routes;
use tokio::sync::RwLock;
use url::Url;
use webauthn_rs::prelude::*;

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct TestApp {
    client: reqwest::Client,
    base_url: String,
    pub token: String,
    _shutdown: tokio::sync::oneshot::Sender<()>,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

impl TestApp {
    async fn new() -> Self {
        let guard = TEST_MUTEX.lock().await;
        InMemoryTaskRepository::shared().clear().await;

        let token = AuthState::generate_token();
        let mut sessions = HashMap::new();
        sessions.insert(token.clone(), 1);
        let mut users = HashMap::new();
        users.insert("test@test.com".to_string(), 1);

        let rp_id = "localhost";
        let rp_origin = Url::parse("http://localhost:3300").expect("invalid URL");
        let webauthn = WebauthnBuilder::new(rp_id, &rp_origin)
            .expect("failed to create webauthn builder")
            .build()
            .expect("failed to build webauthn");

        let state = AuthState {
            webauthn: Arc::new(webauthn),
            reg_states: Arc::new(RwLock::new(HashMap::new())),
            auth_states: Arc::new(RwLock::new(HashMap::new())),
            credentials: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(users)),
            sessions: Arc::new(RwLock::new(sessions)),
            ai_provider: None,
            db: None,
            push: PushService::new(),
        };

        let app = routes::api_routes(&state).with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind");
        let addr = listener.local_addr().expect("no local addr");
        let base_url = format!("http://{addr}");

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    rx.await.ok();
                })
                .await
                .expect("server error");
        });

        Self {
            client: reqwest::Client::new(),
            base_url,
            token,
            _shutdown: tx,
            _guard: guard,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.client.get(self.url(path)).send().await.unwrap()
    }

    async fn post<T: serde::Serialize + Sync>(&self, path: &str, body: &T) -> reqwest::Response {
        self.client
            .post(self.url(path))
            .json(body)
            .send()
            .await
            .unwrap()
    }

    async fn auth_get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(self.url(path))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .unwrap()
    }

    async fn auth_post<T: serde::Serialize + Sync>(
        &self,
        path: &str,
        body: &T,
    ) -> reqwest::Response {
        self.client
            .post(self.url(path))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await
            .unwrap()
    }

    async fn auth_put<T: serde::Serialize + Sync>(
        &self,
        path: &str,
        body: &T,
    ) -> reqwest::Response {
        self.client
            .put(self.url(path))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await
            .unwrap()
    }

    async fn auth_delete(&self, path: &str) -> reqwest::Response {
        self.client
            .delete(self.url(path))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn health_check() {
    let app = TestApp::new().await;
    let res = app.get("/health").await;
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn unauthorized_without_token() {
    let app = TestApp::new().await;
    let res = app
        .post("/api/tasks", &serde_json::json!({"description": "test"}))
        .await;
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn unauthorized_with_bad_token() {
    let app = TestApp::new().await;
    let res = app
        .client
        .get(app.url("/api/tasks"))
        .header("Authorization", "Bearer invalid_token")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn task_crud_full_cycle() {
    let app = TestApp::new().await;

    let create_body = serde_json::json!({
        "description": "Comprar leche",
        "priority": "A",
        "projects": ["casa"],
        "contexts": ["supermercado"],
        "due_date": "2026-07-06"
    });

    let res = app.auth_post("/api/tasks", &create_body).await;
    assert_eq!(res.status(), 200, "create failed");

    let res = app.auth_post("/api/tasks", &create_body).await;
    assert_eq!(res.status(), 200);

    let res = app.auth_get("/api/tasks").await;
    assert_eq!(res.status(), 200);
    let tasks: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(tasks.len(), 2, "expected 2 tasks");

    let task_id = tasks[0]["id"].as_str().unwrap().to_string();

    let res = app.auth_get(&format!("/api/tasks/{task_id}")).await;
    assert_eq!(res.status(), 200);
    let task: serde_json::Value = res.json().await.unwrap();
    assert_eq!(task["description"], "Comprar leche");
    assert_eq!(task["priority"], "A");

    let update_body = serde_json::json!({
        "description": "Comprar pan",
        "priority": "B",
        "projects": [],
        "contexts": []
    });
    let res = app
        .auth_put(&format!("/api/tasks/{task_id}"), &update_body)
        .await;
    assert_eq!(res.status(), 200);
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["description"], "Comprar pan");
    assert_eq!(updated["priority"], "B");

    let res = app.auth_delete(&format!("/api/tasks/{task_id}")).await;
    assert_eq!(res.status(), 204);

    let res = app.auth_get(&format!("/api/tasks/{task_id}")).await;
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn create_task_with_inbox_status() {
    let app = TestApp::new().await;

    let body = serde_json::json!({
        "description": "Tarea desde inbox",
        "projects": [],
        "contexts": []
    });
    let res = app.auth_post("/api/tasks", &body).await;
    assert_eq!(res.status(), 200);
    let task: serde_json::Value = res.json().await.unwrap();
    assert_eq!(task["description"], "Tarea desde inbox");
    assert_eq!(task["status"], "inbox");
}

#[tokio::test]
async fn update_task_status() {
    let app = TestApp::new().await;

    let body = serde_json::json!({
        "description": "Mover a Doing",
        "projects": [],
        "contexts": []
    });
    let res = app.auth_post("/api/tasks", &body).await;
    assert_eq!(res.status(), 200);
    let task: serde_json::Value = res.json().await.unwrap();
    let task_id = task["id"].as_str().unwrap();

    let update_body = serde_json::json!({
        "description": "Mover a Doing",
        "status": "doing",
        "projects": [],
        "contexts": []
    });
    let res = app
        .auth_put(&format!("/api/tasks/{task_id}"), &update_body)
        .await;
    let status = res.status();
    assert_eq!(status, 200, "PUT returned {status}");
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["status"], "doing");
}

#[tokio::test]
async fn delete_nonexistent_task_returns_404() {
    let app = TestApp::new().await;
    let res = app
        .auth_delete("/api/tasks/00000000-0000-0000-0000-000000000000")
        .await;
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn get_nonexistent_task_returns_404() {
    let app = TestApp::new().await;
    let res = app
        .auth_get("/api/tasks/00000000-0000-0000-0000-000000000000")
        .await;
    assert_eq!(res.status(), 404);
}

#[tokio::test]
async fn list_tasks_empty() {
    let app = TestApp::new().await;
    let res = app.auth_get("/api/tasks").await;
    assert_eq!(res.status(), 200);
    let tasks: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(tasks.is_empty());
}
