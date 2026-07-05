use axum::{
    extract::Request,
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::auth::{AuthState, AuthUser};

pub async fn require_auth(
    State(auth_state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let Some(token) = token else {
        tracing::debug!("missing or invalid Authorization header");
        return (
            StatusCode::UNAUTHORIZED,
            "missing or invalid Authorization header",
        )
            .into_response();
    };

    let user_id = {
        let sessions = auth_state.sessions.read().await;
        if let Some(&uid) = sessions.get(token) {
            Some(uid)
        } else {
            None
        }
    };

    let user_id = if let Some(uid) = user_id {
        uid
    } else if let Some(ref db) = auth_state.db {
        if let Ok(Some(uid)) = db.validate_session(token).await {
            auth_state
                .sessions
                .write()
                .await
                .insert(token.to_string(), uid);
            uid
        } else {
            tracing::warn!("invalid session token");
            return (StatusCode::UNAUTHORIZED, "invalid or expired session").into_response();
        }
    } else {
        tracing::warn!("invalid session token");
        return (StatusCode::UNAUTHORIZED, "invalid or expired session").into_response();
    };

    request.extensions_mut().insert(AuthUser { user_id });

    tracing::debug!(user_id, "authenticated request");
    next.run(request).await
}
