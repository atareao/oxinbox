use std::path::PathBuf;

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use oxinbox_backend::{ai, auth, database, db, push, routes};

#[tokio::main]
async fn main() {
    if std::env::var("TRACING_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
            .init();
    } else {
        tracing_subscriber::fmt()
            .pretty()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .init();
    }

    dotenvy::dotenv().ok();
    tracing::info!("oxinbox backend starting");

    let pool = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        let pool = db::create_pool(&database_url)
            .await
            .expect("failed to connect to database");
        db::run_migrations(&pool)
            .await
            .expect("failed to run migrations");
        tracing::info!("database connected and migrated");
        Some(pool)
    } else {
        tracing::warn!("DATABASE_URL not set, running without database");
        None
    };

    let db_repo = pool
        .as_ref()
        .map(|p| database::ParadeDbRepository::arc_new(p.clone()));

    let ai_provider = match ai::create_provider() {
        Ok(provider) => {
            tracing::info!("AI provider ready");
            Some(provider)
        }
        Err(e) => {
            tracing::warn!("AI provider not configured: {e}");
            None
        }
    };

    let push_service = push::PushService::new();
    let auth_state = auth::AuthState::new(ai_provider, db_repo, push_service);
    tracing::info!(
        "WebAuthn configured: rp_id={rp_id}",
        rp_id = std::env::var("RP_ID").unwrap_or_else(|_| "localhost".into())
    );

    push::start_background_worker(auth_state.clone());

    let frontend_dir = std::env::var("FRONTEND_DIR").unwrap_or_else(|_| "frontend/dist".into());
    let frontend_path: PathBuf = frontend_dir.into();
    let frontend_path2 = frontend_path.clone();

    let app = Router::new()
        .merge(routes::api_routes(&auth_state))
        .nest_service("/assets", ServeDir::new(frontend_path.join("assets")))
        .fallback(move |req: Request<Body>| frontend_fallback(req, frontend_path2.clone()))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(auth_state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "3300".into());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");

    tracing::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.expect("server error");
}

async fn frontend_fallback(req: Request<Body>, frontend_path: PathBuf) -> Response {
    let path = req.uri().path().trim_start_matches('/');

    let file_path = frontend_path.join(path);
    if file_path.exists() && file_path.is_file() {
        let Ok(data) = tokio::fs::read(&file_path).await else {
            return (StatusCode::NOT_FOUND, "Not Found").into_response();
        };
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mime = match ext {
            "css" => "text/css; charset=utf-8",
            "js" => "application/javascript; charset=utf-8",
            "json" => "application/json",
            "png" => "image/png",
            "svg" => "image/svg+xml",
            "ico" => "image/x-icon",
            "wasm" => "application/wasm",
            "html" => "text/html; charset=utf-8",
            _ => "application/octet-stream",
        };
        return Response::builder()
            .header("content-type", mime)
            .body(Body::from(data))
            .unwrap_or_else(|_| (StatusCode::NOT_FOUND, "Not Found").into_response());
    }

    let index_path = frontend_path.join("index.html");
    tokio::fs::read_to_string(&index_path).await.map_or_else(
        |_| (StatusCode::NOT_FOUND, "Not Found").into_response(),
        |html| {
            Response::builder()
                .header("content-type", "text/html; charset=utf-8")
                .body(Body::from(html))
                .unwrap_or_else(|_| (StatusCode::NOT_FOUND, "Not Found").into_response())
        },
    )
}
