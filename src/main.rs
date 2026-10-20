use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use sqlx::SqlitePool;

use serde::Deserialize;
use tower_http::trace::TraceLayer;

mod download;
mod serve;
mod types;

#[derive(Clone)]
pub struct SpoolState {
    db: SqlitePool,
    config: Config,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub address: String,
    pub max_file_size_bytes: usize,
    pub storage_path: String,
}

// I want to use the query! macro, but it isn't available using AnyPool
// so I SOMEHOW need to make some dualie Postgres and Sqlite support, but thats future tuxzilla problem
async fn connect_db() -> Result<SqlitePool, sqlx::Error> {
    let database_url = "sqlite://spool.db?mode=rwc";

    SqlitePool::connect(database_url).await
}

async fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = "spool.toml";
    let contents = tokio::fs::read_to_string(path).await?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting spool server...");

    let db = connect_db().await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let state = SpoolState {
        db,
        config: load_config().await?,
    };

    let app = Router::new()
        .route("/", get(|| async { "hello from spool" }))
        .route("/{file_hash}", get(serve::files::serve))
        .route(
            "/upload",
            post(download::files::download)
                .layer(DefaultBodyLimit::max(state.config.max_file_size_bytes)),
        )
        .layer(
            TraceLayer::new_for_http()
                .on_request(
                    |request: &axum::http::Request<axum::body::Body>, _span: &tracing::Span| {
                        tracing::info!(
                            method = %request.method(),
                            uri = %request.uri(),
                            "started processing request"
                        );
                    },
                )
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            status = %response.status(),
                            header_latency = ?latency,
                            "started serving response"
                        );
                    },
                )
                .on_eos(
                    |_trailers: Option<&axum::http::HeaderMap>,
                     stream_latency: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            latency = ?stream_latency,
                            "finished serving response"
                        );
                    },
                ),
        )
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&state.config.address).await?;
    tracing::info!("spool listening on {}", state.config.address);
    axum::serve(listener, app).await?;

    Ok(())
}
