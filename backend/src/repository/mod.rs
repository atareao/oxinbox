use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use oxinbox_core::Task;
use oxinbox_core::Uuid;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::search::{SearchIndex, SearchResult, hybrid_search};

pub type TaskStore = Arc<RwLock<HashMap<Uuid, Task>>>;

static SEARCH_INDEX: LazyLock<RwLock<SearchIndex>> =
    LazyLock::new(|| RwLock::new(SearchIndex::default()));

pub async fn index_task(task: &Task) {
    SEARCH_INDEX.write().await.index_task(task);
}

pub async fn remove_from_index(id: Uuid) {
    SEARCH_INDEX.write().await.remove_task(id);
}

pub async fn store_embedding(task_id: Uuid, embedding: Vec<f32>) {
    SEARCH_INDEX
        .write()
        .await
        .store_embedding(task_id, embedding);
}

pub async fn hybrid_search_tasks(
    tasks: &[Task],
    query: &str,
    query_embedding: Option<&[f32]>,
    limit: usize,
) -> Vec<SearchResult> {
    let idx = SEARCH_INDEX.read().await;
    hybrid_search(tasks, &idx, query, query_embedding, limit, 0.5)
}

pub struct InMemoryTaskRepository {
    tasks: TaskStore,
}

impl InMemoryTaskRepository {
    pub fn new(_owner_id: i32) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn shared() -> &'static Self {
        static REPO: LazyLock<InMemoryTaskRepository> =
            LazyLock::new(|| InMemoryTaskRepository::new(0));
        &REPO
    }

    pub async fn clear(&self) {
        self.tasks.write().await.clear();
    }
}

#[allow(async_fn_in_trait)]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, task: &Task) -> Result<Task, RepositoryError>;
    async fn get(&self, id: Uuid) -> Result<Task, RepositoryError>;
    async fn list(&self, user_id: i32) -> Result<Vec<Task>, RepositoryError>;
    async fn update(&self, task: &Task) -> Result<Task, RepositoryError>;
    async fn delete(&self, id: Uuid) -> Result<(), RepositoryError>;
}

#[derive(thiserror::Error, Debug)]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("task not found: {0}")]
    NotFound(Uuid),
}

impl TaskRepository for InMemoryTaskRepository {
    #[instrument(skip(self), fields(task_id = %task.id))]
    async fn create(&self, task: &Task) -> Result<Task, RepositoryError> {
        tracing::debug!("storing task");
        self.tasks.write().await.insert(task.id, task.clone());
        index_task(task).await;
        Ok(task.clone())
    }

    #[instrument(skip(self), fields(task_id = %id))]
    async fn get(&self, id: Uuid) -> Result<Task, RepositoryError> {
        let result = self
            .tasks
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(RepositoryError::NotFound(id));
        if result.is_ok() {
            tracing::debug!("task found");
        }
        result
    }

    #[instrument(skip(self))]
    async fn list(&self, _: i32) -> Result<Vec<Task>, RepositoryError> {
        let tasks: Vec<Task> = self.tasks.read().await.values().cloned().collect();
        tracing::debug!(count = tasks.len(), "listing tasks");
        Ok(tasks)
    }

    #[instrument(skip(self), fields(task_id = %task.id))]
    async fn update(&self, task: &Task) -> Result<Task, RepositoryError> {
        let mut tasks = self.tasks.write().await;
        if !tasks.contains_key(&task.id) {
            tracing::warn!("task not found for update");
            return Err(RepositoryError::NotFound(task.id));
        }
        let result = task.clone();
        tasks.insert(task.id, result.clone());
        drop(tasks);
        index_task(task).await;
        tracing::debug!("task updated");
        Ok(result)
    }

    #[instrument(skip(self), fields(task_id = %id))]
    async fn delete(&self, id: Uuid) -> Result<(), RepositoryError> {
        let mut tasks = self.tasks.write().await;
        let existed = tasks.remove(&id).is_some();
        drop(tasks);
        if !existed {
            tracing::warn!("task not found for deletion");
            return Err(RepositoryError::NotFound(id));
        }
        remove_from_index(id).await;
        tracing::debug!("task deleted");
        Ok(())
    }
}
