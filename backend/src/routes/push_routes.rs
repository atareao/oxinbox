use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use tracing::instrument;

use crate::auth::{AuthState, AuthUser};

#[derive(Debug, Serialize)]
pub struct VapidKeyResponse {
    public_key: Option<String>,
    configured: bool,
}

#[instrument(skip(state))]
pub async fn vapid_key(State(state): State<AuthState>) -> Json<VapidKeyResponse> {
    Json(VapidKeyResponse {
        public_key: state.push.public_key(),
        configured: state.push.is_configured(),
    })
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[instrument(skip(state))]
pub async fn subscribe_push(
    State(state): State<AuthState>,
    user: axum::Extension<AuthUser>,
    Json(req): Json<SubscribeRequest>,
) -> Result<(), StatusCode> {
    let sub = crate::push::PushSubscription {
        endpoint: req.endpoint,
        p256dh: req.p256dh,
        auth: req.auth,
    };
    state.push.subscribe(&user.user_id, sub).await;
    tracing::info!("push subscription registered");
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeRequest {
    pub endpoint: String,
}

#[instrument(skip(state))]
pub async fn unsubscribe_push(
    State(state): State<AuthState>,
    user: axum::Extension<AuthUser>,
    Json(req): Json<UnsubscribeRequest>,
) -> Result<(), StatusCode> {
    state.push.unsubscribe(&user.user_id, &req.endpoint).await;
    tracing::info!("push subscription removed");
    Ok(())
}
