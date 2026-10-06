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

    Ok(AnyPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .unwrap()) // tuxzilla reminder, add actual error handling
}

#[tokio::main]
async fn main() {
    let db = connect_db().await;

    let app = Router::new()
        .route("/", get(|| async { "hello from spool" }))
        .route("/{file_hash}", get(serve::files::send_image))
        .route(
            "/upload",
            post(download::files::download).layer(DefaultBodyLimit::max(1024 * 1024 * 1024)), // need to be configurable
        )
        .with_state(State { db: db.unwrap() }); // same with this here
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap(); // and here
    axum::serve(listener, app).await.unwrap(); // !!!!!!!!!!!!!!
}
