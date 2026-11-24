use std::{error::Error, str::FromStr, time::Duration};

use sqlx::{
    PgPool, SqlitePool,
    postgres::PgPoolOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

#[derive(Clone)]
pub enum Database {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

pub async fn connect_db(database_url: &str) -> Result<Database, Box<dyn Error>> {
    if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        Ok(Database::Postgres(connect_postgres(database_url).await?))
    } else if database_url.starts_with("sqlite://") {
        Ok(Database::Sqlite(connect_sqlite(database_url).await?))
    } else {
        Err("Unsupported DATABASE_URL".into())
    }
}

async fn connect_sqlite(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(30));

    SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(options)
        .await
}

async fn connect_postgres(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().connect(database_url).await
}
