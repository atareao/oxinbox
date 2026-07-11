use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Inbox,
    Todo,
    Doing,
    Done,
    Someday,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Context {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: Uuid,
    pub completed: bool,
    pub priority: Option<char>,
    pub description: String,
    pub project_ids: Vec<Uuid>,
    pub context_ids: Vec<Uuid>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub due_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskHistoryEntry {
    pub id: i32,
    pub task_id: Uuid,
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub changed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_task(id: Uuid) -> Task {
        Task {
            id,
            completed: false,
            priority: None,
            description: "Test task".into(),
            project_ids: vec![],
            context_ids: vec![],
            status: TaskStatus::Inbox,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            due_date: None,
        }
    }

    #[test]
    fn task_creation_with_uuid_v7() {
        let task = make_task(Uuid::now_v7());
        assert_eq!(task.id.get_version(), Some(uuid::Version::SortRand));
    }

    #[test]
    fn task_serialize_deserialize_roundtrip() {
        let id = Uuid::now_v7();
        let task = Task {
            id,
            completed: false,
            priority: Some('A'),
            description: "Buy milk".into(),
            project_ids: vec![Uuid::now_v7()],
            context_ids: vec![Uuid::now_v7()],
            status: TaskStatus::Todo,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            due_date: Some(NaiveDate::from_ymd_opt(2026, 7, 10).unwrap()),
        };
        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(task.id, deserialized.id);
        assert_eq!(task.description, deserialized.description);
        assert_eq!(task.status, deserialized.status);
        assert_eq!(task.project_ids, deserialized.project_ids);
        assert_eq!(task.context_ids, deserialized.context_ids);
    }

    #[test]
    fn task_status_serialization() {
        let statuses = vec![
            (TaskStatus::Inbox, "\"inbox\""),
            (TaskStatus::Todo, "\"todo\""),
            (TaskStatus::Doing, "\"doing\""),
            (TaskStatus::Done, "\"done\""),
            (TaskStatus::Someday, "\"someday\""),
        ];
        for (status, expected) in statuses {
            assert_eq!(serde_json::to_string(&status).unwrap(), expected);
        }
    }

    #[test]
    fn project_serialize_roundtrip() {
        let p = Project {
            id: Uuid::now_v7(),
            name: "Work".into(),
            color: Some("#1677ff".into()),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(p.id, deserialized.id);
        assert_eq!(p.name, deserialized.name);
        assert_eq!(p.color, deserialized.color);
    }

    #[test]
    fn context_serialize_roundtrip() {
        let c = Context {
            id: Uuid::now_v7(),
            name: "Home".into(),
            color: Some("#52c41a".into()),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let deserialized: Context = serde_json::from_str(&json).unwrap();
        assert_eq!(c.id, deserialized.id);
        assert_eq!(c.name, deserialized.name);
        assert_eq!(c.color, deserialized.color);
    }

    #[test]
    fn task_history_entry_creation() {
        let entry = TaskHistoryEntry {
            id: 1,
            task_id: Uuid::now_v7(),
            field_name: "status".into(),
            old_value: Some("inbox".into()),
            new_value: "doing".into(),
            changed_at: Utc::now(),
        };
        assert_eq!(entry.field_name, "status");
        assert_eq!(entry.old_value, Some("inbox".into()));
        assert_eq!(entry.new_value, "doing");
    }

    #[test]
    fn task_default_status_is_inbox() {
        let task = make_task(Uuid::now_v7());
        assert_eq!(task.status, TaskStatus::Inbox);
        assert!(!task.completed);
    }

    #[test]
    fn completed_task_has_completed_at() {
        let task = Task {
            completed: true,
            completed_at: Some(Utc::now()),
            ..make_task(Uuid::now_v7())
        };
        assert!(task.completed);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn project_default_color_is_none() {
        let p = Project {
            id: Uuid::now_v7(),
            name: "Personal".into(),
            color: None,
            created_at: Utc::now(),
        };
        assert!(p.color.is_none());
    }

    #[test]
    fn context_default_color_is_none() {
        let c = Context {
            id: Uuid::now_v7(),
            name: "Office".into(),
            color: None,
            created_at: Utc::now(),
        };
        assert!(c.color.is_none());
    }
}
