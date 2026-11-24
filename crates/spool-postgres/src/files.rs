// Copyright 2026 Tuxzilla (https://tuxzilla.com)
// SPDX-License-Identifier: Apache-2.0

use sqlx::PgPool;
use sqlx::query;

pub async fn insert_file(
    db: &PgPool,
    hash: &str,
    mimetype: &str,
    file_size_bytes: i64,
) -> Result<bool, sqlx::Error> {
    let result = query!(
        r"
        INSERT INTO files (
            hash_filename,
            mimetype,
            file_size_bytes
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (hash_filename) DO NOTHING
        ",
        hash,
        mimetype,
        file_size_bytes
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

#[allow(clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::insert_file;
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
    async fn insert_and_duplicate_hash_preserves_original_file() {
        let Some(db) = test_db().await else { return };

        assert!(
            insert_file(&db, "file-duplicate", "text/plain", 12)
                .await
                .expect("first insert should succeed")
        );
        assert!(
            !insert_file(&db, "file-duplicate", "image/png", 99)
                .await
                .expect("conflicting insert should be ignored")
        );

        let row: (String, i64) =
            sqlx::query_as("SELECT mimetype, file_size_bytes FROM files WHERE hash_filename = $1")
                .bind("file-duplicate")
                .fetch_one(&db)
                .await
                .expect("original row should remain");
        assert_eq!(row, ("text/plain".to_owned(), 12));
    }
}
