use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::instrument;

#[instrument]
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    tracing::debug!("creating database pool");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;
    tracing::info!("database pool created");
    Ok(pool)
}

#[instrument(skip(pool))]
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    tracing::info!("running database migrations");
    sqlx::migrate!("./migrations").run(pool).await
}
