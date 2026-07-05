use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::instrument;
use url::Url;
use webauthn_rs::prelude::*;

use crate::ai::AiProvider;
use crate::database::ParadeDbRepository;
use crate::push::PushService;

#[derive(Clone)]
pub struct AuthState {
    pub webauthn: Arc<Webauthn>,
    pub reg_states: Arc<RwLock<HashMap<String, (PasskeyRegistration, String)>>>,
    pub auth_states: Arc<RwLock<HashMap<String, (PasskeyAuthentication, i32)>>>,
    pub credentials: Arc<RwLock<HashMap<i32, Vec<Passkey>>>>,
    pub users: Arc<RwLock<HashMap<String, i32>>>,
    pub sessions: Arc<RwLock<HashMap<String, i32>>>,
    pub ai_provider: Option<Arc<dyn AiProvider>>,
    pub db: Option<Arc<ParadeDbRepository>>,
    pub push: PushService,
}

impl AuthState {
    #[instrument(skip(ai_provider, db, push))]
    pub fn new(
        ai_provider: Option<Arc<dyn AiProvider>>,
        db: Option<Arc<ParadeDbRepository>>,
        push: PushService,
    ) -> Self {
        let rp_origin_str =
            std::env::var("RP_ORIGIN").unwrap_or_else(|_| "http://localhost:3300".into());
        let rp_origin = Url::parse(&rp_origin_str).expect("invalid RP_ORIGIN");

        let rp_id = std::env::var("RP_ID").unwrap_or_else(|_| {
            rp_origin.host_str().expect("RP_ORIGIN must have a host").into()
        });

        tracing::debug!(%rp_id, %rp_origin_str, "configuring WebAuthn");

        let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin)
            .expect("failed to create webauthn builder")
            .build()
            .expect("failed to build webauthn");

        Self {
            webauthn: Arc::new(webauthn),
            reg_states: Arc::new(RwLock::new(HashMap::new())),
            auth_states: Arc::new(RwLock::new(HashMap::new())),
            credentials: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ai_provider,
            db,
            push,
        }
    }

    pub fn next_user_id() -> i32 {
        use std::sync::atomic::{AtomicI32, Ordering};
        static COUNTER: AtomicI32 = AtomicI32::new(1);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    pub fn generate_token() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..64)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AuthUser {
    pub user_id: i32,
}
