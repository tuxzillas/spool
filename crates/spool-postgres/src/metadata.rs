// Copyright 2026 Tuxzilla (https://tuxzilla.com)
// SPDX-License-Identifier: Apache-2.0

use sqlx::PgPool;
use sqlx::{FromRow, query_as};

#[derive(Debug, FromRow)]
struct FileMetadata {
    mimetype: String,
}

pub async fn get_file_metadata(db: &PgPool, hash: &str) -> Result<Option<String>, sqlx::Error> {
    let result = query_as!(
        FileMetadata,
        r"
        SELECT mimetype
        FROM files
        WHERE hash_filename = $1
        ",
        hash
    )
    .fetch_optional(db)
    .await?;

    Ok(result.map(|meta| meta.mimetype))
}

#[allow(clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::get_file_metadata;
    use sqlx::{PgPool, postgres::PgPoolOptions};

    async fn test_db() -> Option<PgPool> {
        let database_url = std::env::var("TEST_DATABASE_URL").ok()?;
        let db = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("test database should connect");
        sqlx::migrate!()
            .run(&db)
            .await
            .expect("migrations should succeed");
        Some(db)
    }

    #[tokio::test]
    async fn returns_mimetype_for_known_hash() {
        let Some(db) = test_db().await else { return };
        sqlx::query(
            "INSERT INTO files (hash_filename, mimetype, file_size_bytes) VALUES ($1, $2, $3)",
        )
        .bind("metadata-known")
        .bind("image/png")
        .bind(42_i64)
        .execute(&db)
        .await
        .expect("file fixture should insert");

        let mimetype = get_file_metadata(&db, "metadata-known")
            .await
            .expect("metadata query should succeed");

        assert_eq!(mimetype.as_deref(), Some("image/png"));
    }

    #[tokio::test]
    async fn returns_none_for_unknown_hash() {
        let Some(db) = test_db().await else { return };

        let mimetype = get_file_metadata(&db, "missing")
            .await
            .expect("metadata query should succeed");

        assert_eq!(mimetype, None);
    }
}
