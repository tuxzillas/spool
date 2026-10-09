use axum::{
    Router,
    routing::{get, post},
};

use sqlx::SqlitePool;

use serde::Deserialize;

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
    let db = connect_db().await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let state = SpoolState {
        db,
        config: load_config().await?,
    };

    let app = Router::new()
        .route("/", get(|| async { "hello from spool" }))
        .route("/{file_hash}", get(serve::files::serve))
        .route("/upload", post(download::files::download))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&state.config.address).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
