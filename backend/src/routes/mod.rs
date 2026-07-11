pub mod ai;
pub mod auth;
pub mod contexts;
pub mod geo;
pub mod projects;
pub mod push_routes;
pub mod query;
pub mod tasks;
pub mod voice;

use axum::{Json, middleware};
use serde::Serialize;

use crate::middleware::require_auth;

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
        .route("/auth/login", axum::routing::get(auth::login))
        .route("/auth/callback", axum::routing::get(auth::callback))
        .route("/auth/dev-login", axum::routing::get(auth::dev_login))
        .route(
            "/api/me",
            axum::routing::get(auth::me)
                .layer(middleware::from_fn_with_state(state.clone(), require_auth)),
        )
        .merge(contexts::context_routes(state.clone()))
        .merge(projects::project_routes(state.clone()))
        .merge(tasks::task_routes(state.clone()))
        .merge(ai::ai_routes(state.clone()))
        .merge(query::query_routes(state.clone()))
        .merge(voice::voice_routes())
        .route(
            "/api/push/vapid-key",
            axum::routing::get(push_routes::vapid_key),
        )
        .route(
            "/api/push/subscribe",
            axum::routing::post(push_routes::subscribe_push),
        )
        .route(
            "/api/push/unsubscribe",
            axum::routing::post(push_routes::unsubscribe_push),
        )
        .merge(geo::geo_routes())
}
