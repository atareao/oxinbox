//! End-to-end tests require a running ParadeDB instance.
//!
//! Set `DATABASE_URL` to a test database, then run:
//!
//! ```sh
//! DATABASE_URL=postgres://oxinbox:oxinbox_dev@localhost:5432/oxinbox_test \
//!     cargo test --manifest-path backend/Cargo.toml --test e2e -- --ignored
//! ```
//!
//! These tests are ignored by default because they need a real ParadeDB.
//! They run against an ephemeral test schema (each test gets its own set).

#![allow(clippy::significant_drop_tightening)]

use oxinbox::auth::AuthState;
use oxinbox::database::ParadeDbRepository;
use oxinbox::push::PushService;
use oxinbox::routes;

static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct TestApp {
    client: reqwest::Client,
    base_url: String,
    pub token: String,
    db: ParadeDbRepository,
    _shutdown: tokio::sync::oneshot::Sender<()>,
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

impl TestApp {
    async fn new() -> Self {
        let guard = TEST_MUTEX.lock().await;

        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL required for e2e tests");
        let pool = oxinbox::db::create_pool(&database_url)
            .await
            .expect("failed to connect to ParadeDB");
        oxinbox::db::run_migrations(&pool)
            .await
            .expect("failed to run migrations");

        let db = ParadeDbRepository::new(pool);
        let token = "test-token".to_string();
        let push = PushService::new();
        let state = AuthState::test(None, db, push);

        let app = routes::api_routes(&state).with_state(state.clone());

        // Swap db back in (AuthState::test creates a clone)
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
            db: state.db.as_ref().clone(),
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

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn health_check() {
    let app = TestApp::new().await;
    let res = app.get("/health").await;
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn unauthorized_without_token() {
    let app = TestApp::new().await;
    let res = app
        .post("/api/tasks", &serde_json::json!({"description": "test"}))
        .await;
    assert_eq!(res.status(), 401);
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn task_crud_full_cycle() {
    let app = TestApp::new().await;

    let create_body = serde_json::json!({
        "description": "Comprar leche",
        "priority": "A",
        "project_ids": [],
        "context_ids": [],
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
        "project_ids": [],
        "context_ids": []
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

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn create_task_with_inbox_status() {
    let app = TestApp::new().await;

    let body = serde_json::json!({
        "description": "Tarea desde inbox",
        "project_ids": [],
        "context_ids": []
    });
    let res = app.auth_post("/api/tasks", &body).await;
    assert_eq!(res.status(), 200);
    let task: serde_json::Value = res.json().await.unwrap();
    assert_eq!(task["description"], "Tarea desde inbox");
    assert_eq!(task["status"], "inbox");
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn update_task_status() {
    let app = TestApp::new().await;

    let body = serde_json::json!({
        "description": "Mover a Doing",
        "project_ids": [],
        "context_ids": []
    });
    let res = app.auth_post("/api/tasks", &body).await;
    assert_eq!(res.status(), 200);
    let task: serde_json::Value = res.json().await.unwrap();
    let task_id = task["id"].as_str().unwrap();

    let update_body = serde_json::json!({
        "description": "Mover a Doing",
        "status": "doing",
        "project_ids": [],
        "context_ids": []
    });
    let res = app
        .auth_put(&format!("/api/tasks/{task_id}"), &update_body)
        .await;
    let status = res.status();
    assert_eq!(status, 200, "PUT returned {status}");
    let updated: serde_json::Value = res.json().await.unwrap();
    assert_eq!(updated["status"], "doing");
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn delete_nonexistent_task_returns_404() {
    let app = TestApp::new().await;
    let res = app
        .auth_delete("/api/tasks/00000000-0000-0000-0000-000000000000")
        .await;
    assert_eq!(res.status(), 404);
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn get_nonexistent_task_returns_404() {
    let app = TestApp::new().await;
    let res = app
        .auth_get("/api/tasks/00000000-0000-0000-0000-000000000000")
        .await;
    assert_eq!(res.status(), 404);
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn list_tasks_empty() {
    let app = TestApp::new().await;
    let res = app.auth_get("/api/tasks").await;
    assert_eq!(res.status(), 200);
    let tasks: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(tasks.is_empty());
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn project_crud() {
    let app = TestApp::new().await;

    let res = app.auth_get("/api/projects").await;
    assert_eq!(res.status(), 200);
    let projects: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(projects.is_empty());

    let created = app
        .auth_post(
            "/api/projects",
            &serde_json::json!({"name": "Work", "color": "#1677ff"}),
        )
        .await;
    assert_eq!(created.status(), 200);
    let project: serde_json::Value = created.json().await.unwrap();
    assert_eq!(project["name"], "Work");
    assert_eq!(project["color"], "#1677ff");

    let res = app.auth_get("/api/projects").await;
    assert_eq!(res.status(), 200);
    let projects: Vec<serde_json::Value> = res.json().await.unwrap();
    assert_eq!(projects.len(), 1);

    let id = project["id"].as_str().unwrap();
    let updated = app
        .auth_put(
            &format!("/api/projects/{id}"),
            &serde_json::json!({"name": "Work v2"}),
        )
        .await;
    assert_eq!(updated.status(), 200);

    let res = app.auth_delete(&format!("/api/projects/{id}")).await;
    assert_eq!(res.status(), 204);

    let res = app.auth_get("/api/projects").await;
    assert_eq!(res.status(), 200);
    let projects: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(projects.is_empty());
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn context_crud() {
    let app = TestApp::new().await;

    let created = app
        .auth_post(
            "/api/contexts",
            &serde_json::json!({"name": "Office", "color": "#52c41a"}),
        )
        .await;
    assert_eq!(created.status(), 200);
    let ctx: serde_json::Value = created.json().await.unwrap();
    assert_eq!(ctx["name"], "Office");

    let id = ctx["id"].as_str().unwrap();
    let res = app.auth_get(&format!("/api/contexts/{id}")).await;
    assert_eq!(res.status(), 200);

    let res = app.auth_delete(&format!("/api/contexts/{id}")).await;
    assert_eq!(res.status(), 204);
}

#[ignore = "requires ParadeDB (set DATABASE_URL)"]
#[tokio::test]
async fn task_history_tracks_changes() {
    let app = TestApp::new().await;

    let body = serde_json::json!({
        "description": "Initial",
        "project_ids": [],
        "context_ids": []
    });
    let res = app.auth_post("/api/tasks", &body).await;
    assert_eq!(res.status(), 200);
    let task: serde_json::Value = res.json().await.unwrap();
    let task_id = task["id"].as_str().unwrap();

    let res = app.auth_get(&format!("/api/tasks/{task_id}/history")).await;
    assert_eq!(res.status(), 200);
    let history: Vec<serde_json::Value> = res.json().await.unwrap();
    // No field changes recorded yet — only DB trigger logs status changes
    assert!(
        history.len() <= 1,
        "expected at most 1 trigger-based status entry (created), got {}",
        history.len()
    );

    // Update status to trigger a field_change
    let update = serde_json::json!({
        "description": "Updated",
        "priority": "A",
        "project_ids": [],
        "context_ids": [],
        "status": "doing"
    });
    let res = app
        .auth_put(&format!("/api/tasks/{task_id}"), &update)
        .await;
    assert_eq!(res.status(), 200);

    let res = app.auth_get(&format!("/api/tasks/{task_id}/history")).await;
    assert_eq!(res.status(), 200);
    let history: Vec<serde_json::Value> = res.json().await.unwrap();
    assert!(
        history.len() >= 2,
        "expected at least 2 field_change entries (description + status + priority), got {}",
        history.len()
    );
}
