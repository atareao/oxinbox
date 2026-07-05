use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::instrument;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushError, WebPushMessageBuilder,
};

use crate::auth::AuthState;
use crate::repository::TaskRepository;
use oxinbox_core::TaskStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushSubscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Clone)]
pub struct PushService {
    pub subscriptions: Arc<RwLock<HashMap<i32, Vec<PushSubscription>>>>,
    vapid_private_key: Option<String>,
    vapid_contact: Option<String>,
}

impl PushService {
    pub fn new() -> Self {
        let vapid_private_key = std::env::var("VAPID_PRIVATE_KEY").ok();
        let vapid_contact = std::env::var("VAPID_CONTACT").ok();
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            vapid_private_key,
            vapid_contact,
        }
    }

    pub const fn is_configured(&self) -> bool {
        self.vapid_private_key.is_some()
    }

    #[instrument(skip(self))]
    pub async fn subscribe(&self, user_id: i32, sub: PushSubscription) {
        self.subscriptions
            .write()
            .await
            .entry(user_id)
            .or_default()
            .push(sub);
        tracing::info!(user_id, "push subscription added");
    }

    #[instrument(skip(self))]
    pub async fn unsubscribe(&self, user_id: i32, endpoint: &str) -> bool {
        self.subscriptions
            .write()
            .await
            .get_mut(&user_id)
            .is_some_and(|subs| {
                let before = subs.len();
                subs.retain(|s| s.endpoint != endpoint);
                let removed = subs.len() < before;
                if removed {
                    tracing::info!(user_id, "push subscription removed");
                }
                removed
            })
    }

    #[instrument(skip(self))]
    pub async fn notify_user(&self, user_id: i32, title: &str, body: &str) {
        let Some(ref key) = self.vapid_private_key else {
            tracing::warn!("VAPID not configured, cannot send push");
            return;
        };

        let subs = self.subscriptions.read().await.get(&user_id).cloned();
        let Some(subs) = subs else {
            tracing::debug!(user_id, "no push subscriptions for user");
            return;
        };

        let payload = serde_json::json!({
            "title": title,
            "body": body,
        });
        let payload_bytes = serde_json::to_vec(&payload).unwrap();

        for sub in &subs {
            let info = SubscriptionInfo::new(&sub.endpoint, &sub.p256dh, &sub.auth);
            let sig = match VapidSignatureBuilder::from_base64(key, &info) {
                Ok(mut b) => {
                    if let Some(ref contact) = self.vapid_contact {
                        b.add_claim("sub", contact.as_str());
                    }
                    match b.build() {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(error = %e, "failed to build VAPID signature");
                            continue;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to create VAPID signature builder");
                    continue;
                }
            };

            let mut msg = WebPushMessageBuilder::new(&info);
            msg.set_payload(ContentEncoding::Aes128Gcm, &payload_bytes);
            msg.set_vapid_signature(sig);

            let client = IsahcWebPushClient::new().unwrap();
            match client.send(msg.build().unwrap()).await {
                Ok(()) => tracing::debug!("push sent to {}", sub.endpoint),
                Err(WebPushError::EndpointNotValid(_)) => {
                    tracing::warn!("subscription expired, removing");
                    self.subscriptions
                        .write()
                        .await
                        .entry(user_id)
                        .or_default()
                        .retain(|s| s.endpoint != sub.endpoint);
                }
                Err(WebPushError::ServerError { retry_after, .. }) => {
                    tracing::warn!("push service overloaded, retry after {:?}", retry_after);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "push send failed");
                }
            }
        }
    }
}

pub fn start_background_worker(task_state: AuthState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_hours(1));
        loop {
            interval.tick().await;
            tracing::debug!("background worker: checking stale inbox tasks");

            let tasks = if let Some(ref db) = task_state.db {
                match db.list(0).await {
                    Ok(t) => t,
                    Err(_) => continue,
                }
            } else {
                let repo = crate::repository::InMemoryTaskRepository::shared();
                match repo.list(0).await {
                    Ok(t) => t,
                    Err(_) => continue,
                }
            };

            let now = chrono::Utc::now();
            let stale: Vec<_> = tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Inbox && (now - t.created_at).num_hours() > 24)
                .collect();

            if stale.is_empty() {
                continue;
            }

            tracing::info!(count = stale.len(), "found stale inbox tasks");
            let msg = if stale.len() == 1 {
                format!(
                    "Tienes 1 nota en el Inbox desde hace más de 24h: \"{}\"",
                    stale[0].description
                )
            } else {
                format!(
                    "Tienes {} notas en el Inbox desde hace más de 24h. ¿Las clasificamos?",
                    stale.len()
                )
            };

            let push = PushService::new();
            push.notify_user(0, "oxinbox — Inbox estancado", &msg).await;
        }
    });
}
