use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use tracing::instrument;

use crate::auth::AuthState;

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[instrument(skip(state))]
pub async fn subscribe_push(
    State(state): State<AuthState>,
    Json(req): Json<SubscribeRequest>,
) -> Result<(), StatusCode> {
    let sub = crate::push::PushSubscription {
        endpoint: req.endpoint,
        p256dh: req.p256dh,
        auth: req.auth,
    };
    state.push.subscribe(0, sub).await;
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
    Json(req): Json<UnsubscribeRequest>,
) -> Result<(), StatusCode> {
    state.push.unsubscribe(0, &req.endpoint).await;
    tracing::info!("push subscription removed");
    Ok(())
}