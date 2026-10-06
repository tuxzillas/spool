use sqlx::SqlitePool;
use sqlx::query;

pub async fn insert_file(
    db: &SqlitePool,
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
        VALUES (?, ?, ?)
        ON CONFLICT(hash_filename) DO NOTHING
        ",
        hash,
        mimetype,
        file_size_bytes
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::insert_file;
    use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

    async fn test_db() -> SqlitePool {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database should connect");
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("migrations should succeed");
        db
    }

    #[tokio::test]
    async fn inserts_new_file() {
        let db = test_db().await;

        let inserted = insert_file(&db, "abc123", "text/plain", 12)
            .await
            .expect("file insert should succeed");

        assert!(inserted);
    }

    #[tokio::test]
    async fn duplicate_hash_does_not_replace_file() {
        let db = test_db().await;
        assert!(
            insert_file(&db, "abc123", "text/plain", 12)
                .await
                .expect("first insert should succeed")
        );

        let inserted = insert_file(&db, "abc123", "image/png", 99)
            .await
            .expect("conflicting insert should be ignored");

        assert!(!inserted);
        let row: (String, i64) =
            sqlx::query_as("SELECT mimetype, file_size_bytes FROM files WHERE hash_filename = ?")
                .bind("abc123")
                .fetch_one(&db)
                .await
                .expect("original row should remain");
        assert_eq!(row, ("text/plain".to_owned(), 12));
    }
}
