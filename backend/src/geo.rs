use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLocation {
    pub lat: f64,
    pub lng: f64,
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextZone {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    pub radius_m: f64,
}

#[derive(Clone)]
pub struct GeoService {
    pub locations: Arc<RwLock<HashMap<String, UserLocation>>>,
    pub zones: Arc<RwLock<HashMap<String, ContextZone>>>,
}

impl GeoService {
    pub fn new() -> Self {
        let zones = std::env::var("GEO_ZONES")
            .ok()
            .and_then(|json| {
                serde_json::from_str::<Vec<ContextZone>>(&json)
                    .map_err(|e| tracing::warn!("GEO_ZONES parse error: {e}"))
                    .ok()
            })
            .unwrap_or_default();

        let zone_map = zones.into_iter().map(|z| (z.name.clone(), z)).collect();

        Self {
            locations: Arc::new(RwLock::new(HashMap::new())),
            zones: Arc::new(RwLock::new(zone_map)),
        }
    }

    #[instrument(skip(self))]
    pub async fn update_location(&self, user_id: &str, loc: UserLocation) {
        self.locations.write().await.insert(user_id.to_string(), loc);
        tracing::info!(user_id, "location updated");
    }

    #[instrument(skip(self))]
    pub async fn add_zone(&self, zone: ContextZone) {
        tracing::info!(zone_name = %zone.name, "zone added");
        self.zones.write().await.insert(zone.name.clone(), zone);
    }

    pub async fn check_proximity(&self, user_id: &str) -> Vec<String> {
        let location = self.locations.read().await.get(user_id).cloned();
        let Some(loc) = location else {
            return Vec::new();
        };
        let zones = self.zones.read().await.clone();
        zones
            .into_values()
            .filter(|z| {
                let d = haversine(loc.lat, loc.lng, z.lat, z.lng);
                d <= z.radius_m
            })
            .map(|z| z.name)
            .collect()
    }
}

#[allow(dead_code, clippy::suboptimal_flops)]
fn haversine(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let r = 6_371_000_f64;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lng = (lng2 - lng1).to_radians();
    let a = (d_lat / 2_f64).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lng / 2_f64).sin().powi(2);
    r * 2_f64 * a.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_known_distance() {
        let d = haversine(40.4168, -3.7038, 41.3851, 2.1734);
        assert!((d - 500_000_f64).abs() < 50_000_f64);
    }

    #[test]
    fn haversine_zero_distance() {
        let d = haversine(40.4168, -3.7038, 40.4168, -3.7038);
        assert!(d < 1_f64);
    }

    #[tokio::test]
    async fn check_proximity_empty_when_no_location() {
        let geo = GeoService::new();
        let near = geo.check_proximity("test-user").await;
        assert!(near.is_empty());
    }
}