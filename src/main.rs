use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use sqlx::{AnyPool, any::AnyPoolOptions};

mod download;
mod serve;
mod types;

#[derive(Clone)]
pub struct State {
    db: AnyPool,
}

async fn connect_db() -> Result<AnyPool, sqlx::Error> {
    sqlx::any::install_default_drivers();

    let database_url = "sqlite://spool.db?mode=rwc";

    AnyPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = connect_db().await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let app = Router::new()
        .route("/", get(|| async { "hello from spool" }))
        .route("/{file_hash}", get(serve::files::serve))
        .route(
            "/upload",
            post(download::files::download).layer(DefaultBodyLimit::max(1024 * 1024 * 1024)), // need to be configurable
        )
        .with_state(State { db });
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
