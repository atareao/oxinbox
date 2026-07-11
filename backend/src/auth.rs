use std::sync::Arc;

use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::ai::AiProvider;
use crate::database::ParadeDbRepository;
use crate::push::PushService;

#[derive(Clone, Debug, Deserialize)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl OidcConfig {
    pub fn from_env() -> Option<Self> {
        let issuer = std::env::var("OIDC_ISSUER_URL").ok()?;
        let client_id = std::env::var("OIDC_CLIENT_ID").ok()?;
        let client_secret = std::env::var("OIDC_CLIENT_SECRET").unwrap_or_default();
        let redirect_uri = std::env::var("OIDC_REDIRECT_URI")
            .unwrap_or_else(|_| "http://localhost:3300/auth/callback".into());
        Some(Self {
            issuer,
            client_id,
            client_secret,
            redirect_uri,
        })
    }

    pub fn authorize_url(&self) -> String {
        format!(
            "{}/authorize?response_type=code&client_id={}&redirect_uri={}&scope=openid+profile+email",
            self.issuer, self.client_id, self.redirect_uri
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Jwk {
    pub kty: String,
    pub kid: Option<String>,
    pub n: Option<String>,
    pub e: Option<String>,
    pub alg: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<Jwk>,
}

pub struct JwtValidator {
    jwks: Arc<RwLock<Vec<jsonwebtoken::DecodingKey>>>,
    issuer: String,
    client_id: String,
    is_test: bool,
}

impl JwtValidator {
    pub fn new(issuer: &str, client_id: &str) -> Self {
        Self {
            jwks: Arc::new(RwLock::new(Vec::new())),
            issuer: issuer.to_string(),
            client_id: client_id.to_string(),
            is_test: false,
        }
    }

    pub fn test() -> Self {
        Self {
            jwks: Arc::new(RwLock::new(Vec::new())),
            issuer: "http://localhost:8765".into(),
            client_id: "oxinbox".into(),
            is_test: true,
        }
    }

    pub const fn is_test(&self) -> bool {
        self.is_test
    }

    pub async fn fetch_jwks(&self, issuer: &str) -> Result<(), String> {
        let jwks_url = format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/'));
        let resp: JwksResponse = reqwest::get(&jwks_url)
            .await
            .map_err(|e| format!("failed to fetch JWKS: {e}"))?
            .json()
            .await
            .map_err(|e| format!("failed to parse JWKS: {e}"))?;

        let mut keys = Vec::new();
        for jwk in &resp.keys {
            if let (Some(n), Some(e)) = (&jwk.n, &jwk.e) {
                let n_bytes = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, n)
                    .map_err(|e| format!("invalid JWK n: {e}"))?;
                let e_bytes = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, e)
                    .map_err(|e| format!("invalid JWK e: {e}"))?;
                let key = DecodingKey::from_rsa_raw_components(&n_bytes, &e_bytes);
                keys.push(key);
            }
        }
        tracing::info!(count = keys.len(), "JWKS fetched");
        *self.jwks.write().await = keys;
        Ok(())
    }

    pub async fn validate_token(&self, token: &str) -> Result<JwtClaims, String> {
        let keys = {
            let jwks = self.jwks.read().await;
            if jwks.is_empty() {
                drop(jwks);
                self.fetch_jwks(&self.issuer).await?;
                return Box::pin(self.validate_token(token)).await;
            }
            jwks.clone()
        };

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.client_id]);

        for key in &keys {
            if let Ok(claims) = jsonwebtoken::decode::<JwtClaims>(token, key, &validation) {
                return Ok(claims.claims);
            }
        }
        Err("no matching JWK found for token".to_string())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
}

#[derive(Clone)]
pub struct AuthState {
    pub oidc: Arc<OidcConfig>,
    pub jwt_validator: Arc<JwtValidator>,
    pub ai_provider: Option<Arc<dyn AiProvider>>,
    pub db: Arc<ParadeDbRepository>,
    pub push: PushService,
}

impl AuthState {
    #[instrument(skip(ai_provider, db, push))]
    pub fn new(
        ai_provider: Option<Arc<dyn AiProvider>>,
        db: Arc<ParadeDbRepository>,
        push: PushService,
    ) -> Self {
        let issuer = std::env::var("OIDC_ISSUER_URL").expect("OIDC_ISSUER_URL required");
        let client_id = std::env::var("OIDC_CLIENT_ID").expect("OIDC_CLIENT_ID required");
        Self {
            oidc: Arc::new(OidcConfig::from_env().expect("OIDC_ISSUER_URL and OIDC_CLIENT_ID required")),
            jwt_validator: Arc::new(JwtValidator::new(&issuer, &client_id)),
            ai_provider,
            db,
            push,
        }
    }

    #[doc(hidden)]
    pub fn test(ai_provider: Option<Arc<dyn AiProvider>>, db: Arc<ParadeDbRepository>, push: PushService) -> Self {
        Self {
            oidc: Arc::new(OidcConfig {
                issuer: "http://localhost:8765".into(),
                client_id: "oxinbox".into(),
                client_secret: "test-secret".into(),
                redirect_uri: "http://localhost:3300/auth/callback".into(),
            }),
            jwt_validator: Arc::new(JwtValidator::test()),
            ai_provider,
            db,
            push,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
}