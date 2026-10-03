mod auth;
mod db;
mod error;
mod models;
mod routes;

use std::path::PathBuf;

use axum::{
    extract::DefaultBodyLimit,
    http::header::{HeaderValue, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS},
    routing::{delete, get, post},
    Router,
};
use sqlx::SqlitePool;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub password: String,
    pub love_start: chrono::NaiveDate,
    pub upload_dir: PathBuf,
    pub cookie_secure: bool,
}
fn find_project_root() -> PathBuf {
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    for _ in 0..6 {
        if dir.join("static").join("index.html").is_file() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project_root = find_project_root();
    std::env::set_current_dir(&project_root)?;
    dotenvy::from_path(project_root.join(".env")).ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "love_journal=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let is_loopback = matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1");
    let cookie_secure = std::env::var("COOKIE_SECURE")
        .map(|value| !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "no"))
        .unwrap_or(!is_loopback);
    let password = std::env::var("APP_PASSWORD").map_err(|_| {
        "APP_PASSWORD 未设置：请先复制 .env.example 为 .env，并设置一个至少 8 位的口令"
    })?;
    if password.len() < 8 {
        return Err("APP_PASSWORD 至少需要 8 个字符".into());
    }
    let weak_password = matches!(
        password.as_str(),
        "iloveyou" | "password" | "12345678" | "change-me-please"
    );
    if weak_password && !is_loopback {
        return Err("对外监听时必须先更换 APP_PASSWORD，当前口令过弱".into());
    }
    if weak_password {
        tracing::warn!("APP_PASSWORD 仍是示例或弱口令，请尽快在 .env 中更换");
    }

    let love_start_str =
        std::env::var("LOVE_START").map_err(|_| "LOVE_START 未设置，格式应为 YYYY-MM-DD")?;
    let love_start = chrono::NaiveDate::parse_from_str(&love_start_str, "%Y-%m-%d")
        .map_err(|_| "LOVE_START 格式应为 YYYY-MM-DD")?;

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/journal.db".into());
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "data/uploads".into());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse::<u16>()
        .map_err(|_| "PORT 必须是 1 到 65535 之间的整数")?;

    let upload_dir = PathBuf::from(upload_dir);
    tracing::info!("preparing upload directory");
    std::fs::create_dir_all(&upload_dir)?;
    tracing::info!("upload directory ready");

    if let Some(parent) = PathBuf::from(database_url.trim_start_matches("sqlite://")).parent() {
        tracing::info!("preparing database parent directory");
        std::fs::create_dir_all(parent)?;
        tracing::info!("database parent directory ready");
    }

    let db = db::init(&database_url).await?;

    let state = AppState {
        db,
        password,
        love_start,
        upload_dir: upload_dir.clone(),
        cookie_secure,
    };

    let app = Router::new()
        .route("/healthz", get(routes::health))
        .route("/api/login", post(routes::login))
        .route("/api/logout", post(routes::logout))
        .route("/api/me", get(routes::me))
        .route("/api/stats", get(routes::stats))
        .route("/api/random", get(routes::random_entry))
        .route(
            "/api/entries",
            get(routes::list_entries).post(routes::create_entry),
        )
        .route(
            "/api/entries/:id",
            get(routes::get_entry)
                .put(routes::update_entry)
                .delete(routes::delete_entry),
        )
        .route(
            "/api/entries/:id/photos",
            post(routes::upload_photos).layer(DefaultBodyLimit::max(routes::MAX_UPLOAD_BODY_BYTES)),
        )
        .route("/api/photos/:id", delete(routes::delete_photo))
        .route(
            "/api/entries/:id/comments",
            get(routes::list_comments).post(routes::create_comment),
        )
        .route("/uploads/:filename", get(routes::serve_upload))
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(SetResponseHeaderLayer::overriding(
            X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_ok() {
        tracing::info!("shutdown signal received");
    }
}
