use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::instrument;

#[instrument]
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let max_retries: u32 = std::env::var("DB_CONNECT_RETRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let base_delay_ms: u64 = std::env::var("DB_CONNECT_BASE_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);

    let mut attempt = 0u32;
    loop {
        attempt += 1;
        tracing::info!(attempt, max_retries, "connecting to database");

        match PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await
        {
            Ok(pool) => {
                tracing::info!("database pool created");
                return Ok(pool);
            }
            Err(e) if attempt < max_retries => {
                let delay = Duration::from_millis(base_delay_ms * u64::from(attempt));
                tracing::warn!(
                    attempt,
                    error = %e,
                    retry_in_ms = delay.as_millis(),
                    "database connection failed, retrying"
                );
                tokio::time::sleep(delay).await;
            }
            Err(e) => {
                tracing::error!(attempt, max_retries, error = %e, "all database connection attempts exhausted");
                return Err(e);
            }
        }
    }
}

#[instrument(skip(pool))]
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    tracing::info!("running database migrations");
    sqlx::migrate!("./migrations").run(pool).await
}
