use std::sync::Arc;

use oxinbox_core::{Task, TaskStatus};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::repository::RepositoryError;

pub struct ParadeDbRepository {
    pool: PgPool,
}

impl ParadeDbRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn arc_new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self::new(pool))
    }
}

impl ParadeDbRepository {
    #[instrument(skip(self), fields(task_id = %task.id))]
    pub async fn create(&self, task: &Task, user_id: i32) -> Result<Task, RepositoryError> {
        sqlx::query_as::<_, TaskRow>(
            r"INSERT INTO tasks (id, user_id, completed, priority, description, projects, contexts, status, created_at, updated_at, completed_at, due_date)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               RETURNING id, completed, priority, description, projects, contexts, status, created_at, updated_at, completed_at, due_date"
        )
        .bind(task.id)
        .bind(user_id)
        .bind(task.completed)
        .bind(task.priority.map(|c| c.to_string()))
        .bind(&task.description)
        .bind(&task.projects)
        .bind(&task.contexts)
        .bind(format!("{:?}", task.status).to_lowercase())
        .bind(task.created_at)
        .bind(task.updated_at)
        .bind(task.completed_at)
        .bind(task.due_date)
        .fetch_one(&self.pool)
        .await
        .map(TaskRow::into_task)
        .map_err(RepositoryError::from)
    }

    #[instrument(skip(self), fields(task_id = %id))]
    pub async fn get(&self, id: Uuid) -> Result<Task, RepositoryError> {
        sqlx::query_as::<_, TaskRow>(
            "SELECT id, completed, priority, description, projects, contexts, status, created_at, updated_at, completed_at, due_date FROM tasks WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?
        .map(TaskRow::into_task)
        .ok_or(RepositoryError::NotFound(id))
    }

    #[instrument(skip(self))]
    pub async fn list(&self, user_id: i32) -> Result<Vec<Task>, RepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, completed, priority, description, projects, contexts, status, created_at, updated_at, completed_at, due_date FROM tasks WHERE user_id = $1 ORDER BY created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::from)?;

        Ok(rows.into_iter().map(TaskRow::into_task).collect())
    }

    #[instrument(skip(self), fields(task_id = %task.id))]
    pub async fn update(&self, task: &Task) -> Result<Task, RepositoryError> {
        sqlx::query_as::<_, TaskRow>(
            r"UPDATE tasks SET completed = $1, priority = $2, description = $3, projects = $4, contexts = $5, status = $6, updated_at = $7, completed_at = $8, due_date = $9
               WHERE id = $10
               RETURNING id, completed, priority, description, projects, contexts, status, created_at, updated_at, completed_at, due_date"
        )
        .bind(task.completed)
        .bind(task.priority.map(|c| c.to_string()))
        .bind(&task.description)
        .bind(&task.projects)
        .bind(&task.contexts)
        .bind(format!("{:?}", task.status).to_lowercase())
        .bind(task.updated_at)
        .bind(task.completed_at)
        .bind(task.due_date)
        .bind(task.id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?
        .map(TaskRow::into_task)
        .ok_or(RepositoryError::NotFound(task.id))
    }

    #[instrument(skip(self), fields(task_id = %id))]
    pub async fn delete(&self, id: Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(id));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn search_bm25(
        &self,
        user_id: i32,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Task>, RepositoryError> {
        let like = format!("%{query}%");
        let rows = sqlx::query_as::<_, TaskRow>(
            r"SELECT id, completed, priority, description, projects, contexts, status, created_at, updated_at, completed_at, due_date
               FROM tasks
               WHERE user_id = $1
                 AND (description ILIKE $2 OR $3 = ANY(projects) OR $4 = ANY(contexts))
               ORDER BY created_at DESC
               LIMIT $5"
        )
        .bind(user_id)
        .bind(&like)
        .bind(query)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::from)?;

        Ok(rows.into_iter().map(TaskRow::into_task).collect())
    }

    #[instrument(skip(self))]
    pub async fn search_vector(
        &self,
        user_id: i32,
        embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<(Task, f64)>, RepositoryError> {
        let vec_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        let rows = sqlx::query_as::<_, TaskRowWithScore>(
            r"SELECT id, completed, priority, description, projects, contexts, status, created_at, updated_at, completed_at, due_date,
                     1 - (embedding <=> $1::vector) AS score
               FROM tasks
               WHERE user_id = $2 AND embedding IS NOT NULL
               ORDER BY embedding <=> $1::vector
               LIMIT $3"
        )
        .bind(&vec_str)
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::from)?;

        Ok(rows
            .into_iter()
            .map(|r| -> (Task, f64) {
                let score = r.score;
                (r.into_task(), score)
            })
            .collect())
    }

    #[instrument(skip(self))]
    pub async fn update_embedding(
        &self,
        task_id: Uuid,
        embedding: &[f32],
    ) -> Result<(), RepositoryError> {
        let vec_str = format!(
            "[{}]",
            embedding
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        sqlx::query("UPDATE tasks SET embedding = $1::vector WHERE id = $2")
            .bind(&vec_str)
            .bind(task_id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn upsert_user(&self, email: &str) -> Result<i32, RepositoryError> {
        let row: (i32,) = sqlx::query_as(
            "INSERT INTO users (email) VALUES ($1) ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email RETURNING id"
        )
        .bind(email)
        .fetch_one(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(row.0)
    }

    #[instrument(skip(self))]
    pub async fn create_session(&self, token: &str, user_id: i32) -> Result<(), RepositoryError> {
        let expires = chrono::Utc::now() + chrono::TimeDelta::try_days(365).unwrap();
        sqlx::query("INSERT INTO sessions (token, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(token)
            .bind(user_id)
            .bind(expires)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn validate_session(&self, token: &str) -> Result<Option<i32>, RepositoryError> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT user_id FROM sessions WHERE token = $1 AND expires_at > CURRENT_TIMESTAMP",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(row.map(|r| r.0))
    }
}

#[derive(sqlx::FromRow)]
struct TaskRowWithScore {
    id: Uuid,
    completed: bool,
    priority: Option<String>,
    description: String,
    projects: Vec<String>,
    contexts: Vec<String>,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    due_date: Option<chrono::NaiveDate>,
    score: f64,
}

impl TaskRowWithScore {
    fn into_task(self) -> Task {
        let status = match self.status.as_str() {
            "inbox" => TaskStatus::Inbox,
            "todo" => TaskStatus::Todo,
            "doing" => TaskStatus::Doing,
            "done" => TaskStatus::Done,
            _ => TaskStatus::Someday,
        };
        Task {
            id: self.id,
            completed: self.completed,
            priority: self
                .priority
                .and_then(|p| p.chars().next())
                .filter(char::is_ascii_uppercase),
            description: self.description,
            projects: self.projects,
            contexts: self.contexts,
            status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            due_date: self.due_date,
        }
    }
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    completed: bool,
    priority: Option<String>,
    description: String,
    projects: Vec<String>,
    contexts: Vec<String>,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    due_date: Option<chrono::NaiveDate>,
}

impl TaskRow {
    fn into_task(self) -> Task {
        let status = match self.status.as_str() {
            "inbox" => TaskStatus::Inbox,
            "todo" => TaskStatus::Todo,
            "doing" => TaskStatus::Doing,
            "done" => TaskStatus::Done,
            _ => TaskStatus::Someday,
        };
        Task {
            id: self.id,
            completed: self.completed,
            priority: self
                .priority
                .and_then(|p| p.chars().next())
                .filter(char::is_ascii_uppercase),
            description: self.description,
            projects: self.projects,
            contexts: self.contexts,
            status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            due_date: self.due_date,
        }
    }
}
