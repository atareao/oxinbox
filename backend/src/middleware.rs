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
        return (
            StatusCode::UNAUTHORIZED,
            "missing or invalid Authorization header",
        )
            .into_response();
    };

    let token = token.to_string();
    let user = if auth_state.jwt_validator.is_test() || std::env::var("DEV_MODE").is_ok() {
        tracing::debug!("dev-mode auth bypass for user_id={token}");
        AuthUser {
            user_id: token,
            email: Some("dev@oxinbox.app".into()),
            name: Some("Dev User".into()),
        }
    } else {
        match auth_state.jwt_validator.validate_token(&token).await {
            Ok(claims) => {
                tracing::debug!("authenticated request");
                AuthUser {
                    user_id: claims.sub,
                    email: claims.email,
                    name: claims.name,
                }
            }
            Err(e) => {
                tracing::warn!("JWT validation failed: {e}");
                return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
            }
        }
    };

    request.extensions_mut().insert(user);
    next.run(request).await
}
