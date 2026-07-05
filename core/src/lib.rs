use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

pub use uuid::Uuid;

pub use chrono;
pub use serde;
pub use uuid;

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
pub struct Task {
    pub id: Uuid,
    pub completed: bool,
    pub priority: Option<char>,
    pub description: String,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub due_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskHistory {
    pub id: i32,
    pub task_id: Uuid,
    pub from_status: Option<TaskStatus>,
    pub to_status: TaskStatus,
    pub changed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_creation_with_uuid_v7() {
        let task = Task {
            id: Uuid::now_v7(),
            completed: false,
            priority: None,
            description: "Test task".into(),
            projects: vec![],
            contexts: vec![],
            status: TaskStatus::Inbox,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            due_date: None,
        };
        assert_eq!(task.id.get_version(), Some(uuid::Version::SortRand));
    }

    #[test]
    fn task_serialize_deserialize_roundtrip() {
        let task = Task {
            id: Uuid::now_v7(),
            completed: false,
            priority: Some('A'),
            description: "Buy milk +proyecto @casa".into(),
            projects: vec!["proyecto".into()],
            contexts: vec!["casa".into()],
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
    fn task_history_creation() {
        let history = TaskHistory {
            id: 1,
            task_id: Uuid::now_v7(),
            from_status: Some(TaskStatus::Inbox),
            to_status: TaskStatus::Doing,
            changed_at: Utc::now(),
        };
        assert_eq!(history.from_status, Some(TaskStatus::Inbox));
        assert_eq!(history.to_status, TaskStatus::Doing);
    }

    #[test]
    fn task_default_status_is_inbox() {
        let task = Task {
            id: Uuid::now_v7(),
            completed: false,
            priority: None,
            description: "test".into(),
            projects: vec![],
            contexts: vec![],
            status: TaskStatus::Inbox,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            due_date: None,
        };
        assert_eq!(task.status, TaskStatus::Inbox);
        assert!(!task.completed);
    }

    #[test]
    fn completed_task_has_completed_at() {
        let task = Task {
            id: Uuid::now_v7(),
            completed: true,
            priority: None,
            description: "Done task".into(),
            projects: vec![],
            contexts: vec![],
            status: TaskStatus::Done,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: Some(Utc::now()),
            due_date: None,
        };
        assert!(task.completed);
        assert!(task.completed_at.is_some());
    }
}
