use crate::core_types::Uuid;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("not found: {0}")]
    NotFound(Uuid),
}