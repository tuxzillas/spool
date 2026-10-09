use axum::{
    Router,
    routing::{get, post},
};

use sqlx::SqlitePool;

mod download;
mod serve;
mod types;

#[derive(Clone)]
pub struct SpoolState {
    db: SqlitePool,
    config: Config,
}

#[derive(Clone)]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = connect_db().await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let state = SpoolState {
        db,
        config: Config {
            address: "0.0.0.0:3000".to_string(),
            max_file_size_bytes: 1024 * 1024 * 1024,
            storage_path: "/home/tuxzilla/Projects/spool-storage".to_string(),
        },
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
