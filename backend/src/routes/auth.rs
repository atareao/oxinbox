use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use webauthn_rs::prelude::*;

use crate::auth::AuthState;

#[derive(Debug, Deserialize)]
pub struct StartRegistrationRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct StartRegistrationResponse {
    pub challenge: CreationChallengeResponse,
    pub state_id: String,
}

#[derive(Debug, Deserialize)]
pub struct FinishRegistrationRequest {
    pub state_id: String,
    pub credential: RegisterPublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub struct FinishRegistrationResponse {
    pub token: String,
    pub user_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct StartLoginRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct StartLoginResponse {
    pub challenge: RequestChallengeResponse,
    pub state_id: String,
}

#[derive(Debug, Deserialize)]
pub struct FinishLoginRequest {
    pub state_id: String,
    pub credential: PublicKeyCredential,
}

#[derive(Debug, Serialize)]
pub struct FinishLoginResponse {
    pub token: String,
    pub user_id: i32,
}

#[instrument(skip(state), fields(email = %req.email))]
pub async fn start_registration(
    State(state): State<AuthState>,
    Json(req): Json<StartRegistrationRequest>,
) -> Result<Json<StartRegistrationResponse>, StatusCode> {
    let user_uuid = Uuid::new_v4();
    let (ccr, reg_state) = state
        .webauthn
        .start_passkey_registration(user_uuid, &req.email, &req.email, None)
        .map_err(|e| {
            tracing::error!(error = %e, "start_passkey_registration failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let state_id = AuthState::generate_token();
    state
        .reg_states
        .write()
        .await
        .insert(state_id.clone(), (reg_state, req.email));

    tracing::info!("registration challenge issued");
    Ok(Json(StartRegistrationResponse {
        challenge: ccr,
        state_id,
    }))
}

#[instrument(skip(state), fields(state_id = %req.state_id))]
pub async fn finish_registration(
    State(state): State<AuthState>,
    Json(req): Json<FinishRegistrationRequest>,
) -> Result<Json<FinishRegistrationResponse>, StatusCode> {
    let (reg_state, email) = state
        .reg_states
        .write()
        .await
        .remove(&req.state_id)
        .ok_or_else(|| {
            tracing::warn!("registration state not found or expired");
            StatusCode::BAD_REQUEST
        })?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&req.credential, &reg_state)
        .map_err(|e| {
            tracing::error!(error = %e, "finish_passkey_registration failed");
            StatusCode::UNAUTHORIZED
        })?;

    let user_id = AuthState::next_user_id();
    state.users.write().await.insert(email.clone(), user_id);
    state
        .credentials
        .write()
        .await
        .insert(user_id, vec![passkey]);

    let token = AuthState::generate_token();
    state.sessions.write().await.insert(token.clone(), user_id);

    if let Some(ref db) = state.db {
        let _ = db.upsert_user(&email).await;
        let _ = db.create_session(&token, user_id).await;
    }

    tracing::info!(user_id, "user registered and session created");
    Ok(Json(FinishRegistrationResponse { token, user_id }))
}

#[instrument(skip(state), fields(email = %req.email))]
pub async fn start_login(
    State(state): State<AuthState>,
    Json(req): Json<StartLoginRequest>,
) -> Result<Json<StartLoginResponse>, StatusCode> {
    let user_id = state
        .users
        .read()
        .await
        .get(&req.email)
        .copied()
        .ok_or_else(|| {
            tracing::warn!("login attempt for unknown email");
            StatusCode::NOT_FOUND
        })?;

    let credentials = state.credentials.read().await;
    let Some(user_credentials) = credentials.get(&user_id) else {
        return Err(StatusCode::NOT_FOUND);
    };
    let user_credentials = user_credentials.clone();
    drop(credentials);

    let (rcr, auth_state) = state
        .webauthn
        .start_passkey_authentication(&user_credentials)
        .map_err(|e| {
            tracing::error!(error = %e, "start_passkey_authentication failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let state_id = AuthState::generate_token();
    state
        .auth_states
        .write()
        .await
        .insert(state_id.clone(), (auth_state, user_id));

    tracing::info!(user_id, "authentication challenge issued");
    Ok(Json(StartLoginResponse {
        challenge: rcr,
        state_id,
    }))
}

#[instrument(skip(state), fields(state_id = %req.state_id))]
pub async fn finish_login(
    State(state): State<AuthState>,
    Json(req): Json<FinishLoginRequest>,
) -> Result<Json<FinishLoginResponse>, StatusCode> {
    let (auth_state, user_id) = state
        .auth_states
        .write()
        .await
        .remove(&req.state_id)
        .ok_or_else(|| {
            tracing::warn!("authentication state not found or expired");
            StatusCode::BAD_REQUEST
        })?;

    let _auth_result = state
        .webauthn
        .finish_passkey_authentication(&req.credential, &auth_state)
        .map_err(|e| {
            tracing::error!(error = %e, "finish_passkey_authentication failed");
            StatusCode::UNAUTHORIZED
        })?;

    let token = AuthState::generate_token();
    state.sessions.write().await.insert(token.clone(), user_id);

    if let Some(ref db) = state.db {
        let _ = db.create_session(&token, user_id).await;
    }

    tracing::info!(user_id, "user authenticated and session created");
    Ok(Json(FinishLoginResponse { token, user_id }))
}
