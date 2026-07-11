use axum::{
    Json,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::auth::AuthState;

#[derive(Debug, Deserialize)]
pub struct AuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DevLoginQuery {
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    expires_in: u64,
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    sub: String,
    email: Option<String>,
    name: Option<String>,
}

#[instrument(skip(state))]
pub async fn login(State(state): State<AuthState>) -> Redirect {
    let url = state.oidc.authorize_url();
    tracing::info!("redirecting to OIDC provider: {}", url);
    Redirect::to(&url)
}

#[instrument(skip(state))]
pub async fn callback(
    State(state): State<AuthState>,
    Query(query): Query<AuthCallbackQuery>,
) -> impl IntoResponse {
    let token_url = format!("{}/api/oidc/token", state.oidc.issuer.trim_end_matches('/'));

    let params = [
        ("grant_type", "authorization_code"),
        ("code", &query.code),
        ("redirect_uri", &state.oidc.redirect_uri),
        ("client_id", &state.oidc.client_id),
        ("client_secret", &state.oidc.client_secret),
    ];

    let client = reqwest::Client::new();
    let token_resp = match client.post(&token_url).form(&params).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, url = %token_url, "token exchange failed");
            return (
                StatusCode::BAD_GATEWAY,
                format!("token exchange failed: {e}"),
            )
                .into_response();
        }
    };

    let status = token_resp.status();
    if !status.is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        tracing::error!("token endpoint error {}: {}", status, body);
        return (
            StatusCode::BAD_GATEWAY,
            format!("token endpoint error: {body}"),
        )
            .into_response();
    }

    let token_data: TokenResponse = match token_resp.json().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("failed to parse token response: {e}");
            return (StatusCode::BAD_GATEWAY, "invalid token response").into_response();
        }
    };

    let access_token = token_data.access_token.clone();
    let jwt = token_data.id_token.unwrap_or(token_data.access_token);

    let userinfo_url = format!(
        "{}/api/oidc/userinfo",
        state.oidc.issuer.trim_end_matches('/')
    );

    let user_info = match client
        .get(&userinfo_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
    {
        Ok(r) => r.json::<UserInfoResponse>().await.ok(),
        Err(_) => None,
    };

    let html = format!(
        r"<!DOCTYPE html>
<html>
<head><title>Redirecting...</title></head>
<body>
<script>
sessionStorage.setItem('oxinbox_token', '{jwt}');
{user_data}
window.location.href = '/';
</script>
</body>
</html>",
        jwt = jwt,
        user_data = user_info
            .as_ref()
            .map(|u| {
                format!(
                    "sessionStorage.setItem('oxinbox_user', JSON.stringify({}));",
                    serde_json::json!({
                        "sub": u.sub,
                        "email": u.email,
                        "name": u.name,
                    })
                )
            })
            .unwrap_or_default()
    );

    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

#[instrument(skip(state))]
pub async fn dev_login(
    State(state): State<AuthState>,
    Query(query): Query<DevLoginQuery>,
) -> impl IntoResponse {
    let sub = query
        .email
        .clone()
        .unwrap_or_else(|| "dev@oxinbox.app".into());

    let jwt = if state.oidc.client_secret.is_empty() {
        state.oidc.client_id.clone()
    } else {
        sub.clone()
    };

    let html = format!(
        r"<!DOCTYPE html>
<html>
<head><title>Redirecting...</title></head>
<body>
<script>
sessionStorage.setItem('oxinbox_token', '{jwt}');
sessionStorage.setItem('oxinbox_user', JSON.stringify({user}));
window.location.href = '/';
</script>
</body>
</html>",
        jwt = jwt,
        user = serde_json::json!({
            "sub": sub,
            "email": sub,
            "name": sub.split('@').next().unwrap_or("Dev"),
        })
    );

    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

pub async fn me(axum::Extension(user): axum::Extension<crate::auth::AuthUser>) -> Json<MeResponse> {
    Json(MeResponse {
        sub: user.user_id,
        email: user.email,
        name: user.name,
    })
}
