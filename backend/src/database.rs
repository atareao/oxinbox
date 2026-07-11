use std::sync::Arc;

use crate::core_types::{Context, Project, Task, TaskHistoryEntry, TaskStatus};
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

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------
impl ParadeDbRepository {
    #[instrument(skip(self), fields(task_id = %task.id))]
    pub async fn create(&self, task: &Task, user_id: &str) -> Result<Task, RepositoryError> {
        sqlx::query_as::<_, TaskRow>(
            r"INSERT INTO tasks (id, user_id, completed, priority, description, project_ids, context_ids, status, created_at, updated_at, completed_at, due_date)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
               RETURNING id, completed, priority, description, project_ids, context_ids, status, created_at, updated_at, completed_at, due_date",
        )
        .bind(task.id)
        .bind(user_id)
        .bind(task.completed)
        .bind(task.priority.map(|c| c.to_string()))
        .bind(&task.description)
        .bind(&task.project_ids)
        .bind(&task.context_ids)
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
            "SELECT id, completed, priority, description, project_ids, context_ids, status, created_at, updated_at, completed_at, due_date FROM tasks WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?
        .map(TaskRow::into_task)
        .ok_or(RepositoryError::NotFound(id))
    }

    #[instrument(skip(self))]
    pub async fn list(&self, user_id: &str) -> Result<Vec<Task>, RepositoryError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, completed, priority, description, project_ids, context_ids, status, created_at, updated_at, completed_at, due_date FROM tasks WHERE user_id = $1 ORDER BY created_at DESC",
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
            r"UPDATE tasks SET completed = $1, priority = $2, description = $3, project_ids = $4, context_ids = $5, status = $6, updated_at = $7, completed_at = $8, due_date = $9
               WHERE id = $10
               RETURNING id, completed, priority, description, project_ids, context_ids, status, created_at, updated_at, completed_at, due_date",
        )
        .bind(task.completed)
        .bind(task.priority.map(|c| c.to_string()))
        .bind(&task.description)
        .bind(&task.project_ids)
        .bind(&task.context_ids)
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
        user_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<Task>, RepositoryError> {
        let like = format!("%{query}%");
        let rows = sqlx::query_as::<_, TaskRow>(
            r"SELECT id, completed, priority, description, project_ids, context_ids, status, created_at, updated_at, completed_at, due_date
               FROM tasks
               WHERE user_id = $1
                 AND (description ILIKE $2)
               ORDER BY created_at DESC
               LIMIT $3",
        )
        .bind(user_id)
        .bind(&like)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::from)?;

        Ok(rows.into_iter().map(TaskRow::into_task).collect())
    }

    #[instrument(skip(self))]
    pub async fn search_vector(
        &self,
        user_id: &str,
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
            r"SELECT id, completed, priority, description, project_ids, context_ids, status, created_at, updated_at, completed_at, due_date,
                     1 - (embedding <=> $1::vector) AS score
               FROM tasks
               WHERE user_id = $2 AND embedding IS NOT NULL
               ORDER BY embedding <=> $1::vector
               LIMIT $3",
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
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------
impl ParadeDbRepository {
    pub async fn list_projects(&self) -> Result<Vec<Project>, RepositoryError> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, color, created_at FROM projects ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(rows.into_iter().map(ProjectRow::into_project).collect())
    }

    pub async fn create_project(&self, project: &Project) -> Result<Project, RepositoryError> {
        sqlx::query_as::<_, ProjectRow>(
            r"INSERT INTO projects (id, name, color, created_at)
               VALUES ($1, $2, $3, $4)
               RETURNING id, name, color, created_at",
        )
        .bind(project.id)
        .bind(&project.name)
        .bind(&project.color)
        .bind(project.created_at)
        .fetch_one(&self.pool)
        .await
        .map(ProjectRow::into_project)
        .map_err(RepositoryError::from)
    }

    pub async fn get_project(&self, id: Uuid) -> Result<Project, RepositoryError> {
        sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, color, created_at FROM projects WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?
        .map(ProjectRow::into_project)
        .ok_or(RepositoryError::NotFound(id))
    }

    pub async fn update_project(&self, project: &Project) -> Result<Project, RepositoryError> {
        sqlx::query_as::<_, ProjectRow>(
            r"UPDATE projects SET name = $1, color = $2 WHERE id = $3
               RETURNING id, name, color, created_at",
        )
        .bind(&project.name)
        .bind(&project.color)
        .bind(project.id)
        .fetch_one(&self.pool)
        .await
        .map(ProjectRow::into_project)
        .map_err(RepositoryError::from)
    }

    pub async fn delete_project(&self, id: Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(id));
        }
        Ok(())
    }

    pub async fn find_project_by_name(&self, name: &str) -> Result<Option<Project>, RepositoryError> {
        let row = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, name, color, created_at FROM projects WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(row.map(ProjectRow::into_project))
    }

    pub async fn create_or_find_project(&self, name: &str) -> Result<Project, RepositoryError> {
        if let Some(project) = self.find_project_by_name(name).await? {
            return Ok(project);
        }
        let project = Project {
            id: Uuid::now_v7(),
            name: name.to_string(),
            color: None,
            created_at: chrono::Utc::now(),
        };
        self.create_project(&project).await
    }
}

// ---------------------------------------------------------------------------
// Contexts
// ---------------------------------------------------------------------------
impl ParadeDbRepository {
    pub async fn list_contexts(&self) -> Result<Vec<Context>, RepositoryError> {
        let rows = sqlx::query_as::<_, ContextRow>(
            "SELECT id, name, color, created_at FROM contexts ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(rows.into_iter().map(ContextRow::into_context).collect())
    }

    pub async fn create_context(&self, ctx: &Context) -> Result<Context, RepositoryError> {
        sqlx::query_as::<_, ContextRow>(
            r"INSERT INTO contexts (id, name, color, created_at)
               VALUES ($1, $2, $3, $4)
               RETURNING id, name, color, created_at",
        )
        .bind(ctx.id)
        .bind(&ctx.name)
        .bind(&ctx.color)
        .bind(ctx.created_at)
        .fetch_one(&self.pool)
        .await
        .map(ContextRow::into_context)
        .map_err(RepositoryError::from)
    }

    pub async fn get_context(&self, id: Uuid) -> Result<Context, RepositoryError> {
        sqlx::query_as::<_, ContextRow>(
            "SELECT id, name, color, created_at FROM contexts WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?
        .map(ContextRow::into_context)
        .ok_or(RepositoryError::NotFound(id))
    }

    pub async fn update_context(&self, ctx: &Context) -> Result<Context, RepositoryError> {
        sqlx::query_as::<_, ContextRow>(
            r"UPDATE contexts SET name = $1, color = $2 WHERE id = $3
               RETURNING id, name, color, created_at",
        )
        .bind(&ctx.name)
        .bind(&ctx.color)
        .bind(ctx.id)
        .fetch_one(&self.pool)
        .await
        .map(ContextRow::into_context)
        .map_err(RepositoryError::from)
    }

    pub async fn delete_context(&self, id: Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("DELETE FROM contexts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(id));
        }
        Ok(())
    }

    pub async fn find_context_by_name(&self, name: &str) -> Result<Option<Context>, RepositoryError> {
        let row = sqlx::query_as::<_, ContextRow>(
            "SELECT id, name, color, created_at FROM contexts WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(row.map(ContextRow::into_context))
    }

    pub async fn create_or_find_context(&self, name: &str) -> Result<Context, RepositoryError> {
        if let Some(ctx) = self.find_context_by_name(name).await? {
            return Ok(ctx);
        }
        let ctx = Context {
            id: Uuid::now_v7(),
            name: name.to_string(),
            color: None,
            created_at: chrono::Utc::now(),
        };
        self.create_context(&ctx).await
    }
}

// ---------------------------------------------------------------------------
// Field changes (rich history)
// ---------------------------------------------------------------------------
impl ParadeDbRepository {
    pub async fn insert_field_change(
        &self,
        entry: &TaskHistoryEntry,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r"INSERT INTO field_changes (task_id, field_name, old_value, new_value, changed_at)
               VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(entry.task_id)
        .bind(&entry.field_name)
        .bind(&entry.old_value)
        .bind(&entry.new_value)
        .bind(entry.changed_at)
        .execute(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(())
    }

    pub async fn list_field_changes(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<TaskHistoryEntry>, RepositoryError> {
        let rows = sqlx::query_as::<_, FieldChangeRow>(
            r"SELECT id, task_id, field_name, old_value, new_value, changed_at
               FROM field_changes
               WHERE task_id = $1
               ORDER BY changed_at",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(RepositoryError::from)?;
        Ok(rows.into_iter().map(FieldChangeRow::into_entry).collect())
    }
}

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct TaskRowWithScore {
    id: Uuid,
    completed: bool,
    priority: Option<String>,
    description: String,
    project_ids: Vec<Uuid>,
    context_ids: Vec<Uuid>,
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
            project_ids: self.project_ids,
            context_ids: self.context_ids,
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
    project_ids: Vec<Uuid>,
    context_ids: Vec<Uuid>,
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
            project_ids: self.project_ids,
            context_ids: self.context_ids,
            status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            due_date: self.due_date,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: Uuid,
    name: String,
    color: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl ProjectRow {
    fn into_project(self) -> Project {
        Project {
            id: self.id,
            name: self.name,
            color: self.color,
            created_at: self.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ContextRow {
    id: Uuid,
    name: String,
    color: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl ContextRow {
    fn into_context(self) -> Context {
        Context {
            id: self.id,
            name: self.name,
            color: self.color,
            created_at: self.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct FieldChangeRow {
    id: i32,
    task_id: Uuid,
    field_name: String,
    old_value: Option<String>,
    new_value: String,
    changed_at: chrono::DateTime<chrono::Utc>,
}

impl FieldChangeRow {
    fn into_entry(self) -> TaskHistoryEntry {
        TaskHistoryEntry {
            id: self.id,
            task_id: self.task_id,
            field_name: self.field_name,
            old_value: self.old_value,
            new_value: self.new_value,
            changed_at: self.changed_at,
        }
    }
}