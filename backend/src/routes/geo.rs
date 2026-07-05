use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use tracing::instrument;

use crate::auth::{AuthState, AuthUser};
use crate::geo::{GeoService, UserLocation};
use crate::repository::InMemoryTaskRepository;
use crate::repository::TaskRepository;

#[derive(Debug, Deserialize)]
pub struct LocationUpdate {
    pub lat: f64,
    pub lng: f64,
    pub accuracy: f64,
}

#[instrument(skip(state))]
pub async fn update_location(
    State(state): State<AuthState>,
    _: axum::Extension<AuthUser>,
    Json(req): Json<LocationUpdate>,
) -> Result<(), String> {
    let geo = GeoService::new();
    geo.update_location(
        0,
        UserLocation {
            lat: req.lat,
            lng: req.lng,
            accuracy: req.accuracy,
        },
    )
    .await;

    let zone_names = geo.check_proximity(0).await;
    if zone_names.is_empty() {
        tracing::info!("location updated, no zones nearby");
        return Ok(());
    }

    let tasks = if let Some(ref db) = state.db {
        db.list(0).await.map_err(|e| e.to_string())?
    } else {
        InMemoryTaskRepository::shared()
            .list(0)
            .await
            .map_err(|e| e.to_string())?
    };

    for zone in &zone_names {
        let matching: Vec<_> = tasks
            .iter()
            .filter(|t| t.contexts.iter().any(|c| c == zone))
            .collect();

        if !matching.is_empty() {
            let body = if matching.len() == 1 {
                format!(
                    "Tienes \"{}\" pendiente en @{zone}",
                    matching[0].description
                )
            } else {
                format!("Tienes {} tareas pendientes en @{zone}", matching.len())
            };
            state
                .push
                .notify_user(0, "oxinbox — Cerca de zona", &body)
                .await;
        }
    }

    tracing::info!("location updated, zones nearby: {:?}", zone_names);
    Ok(())
}

pub fn geo_routes() -> axum::Router<crate::auth::AuthState> {
    axum::Router::new().route("/api/location", axum::routing::post(update_location))
}
