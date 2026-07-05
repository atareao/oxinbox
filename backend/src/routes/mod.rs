pub mod ai;
pub mod auth;
pub mod geo;
pub mod push_routes;
pub mod query;
pub mod tasks;
pub mod voice;

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub fn api_routes(state: &crate::auth::AuthState) -> axum::Router<crate::auth::AuthState> {
    axum::Router::new()
        .route("/health", axum::routing::get(health))
        .merge(tasks::task_routes(state.clone()))
        .merge(ai::ai_routes(state.clone()))
        .merge(query::query_routes(state.clone()))
        .merge(voice::voice_routes())
        .route(
            "/api/push/subscribe",
            axum::routing::post(push_routes::subscribe_push),
        )
        .route(
            "/api/push/unsubscribe",
            axum::routing::post(push_routes::unsubscribe_push),
        )
        .merge(geo::geo_routes())
        .route(
            "/auth/register/start",
            axum::routing::post(auth::start_registration),
        )
        .route(
            "/auth/register/finish",
            axum::routing::post(auth::finish_registration),
        )
        .route("/auth/login/start", axum::routing::post(auth::start_login))
        .route(
            "/auth/login/finish",
            axum::routing::post(auth::finish_login),
        )
}
