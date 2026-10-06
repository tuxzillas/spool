use sqlx::SqlitePool;
use sqlx::{FromRow, query_as};

#[derive(Debug, FromRow)]
struct FileMetadata {
    mimetype: String,
}

pub async fn get_file_mimetype(db: &SqlitePool, hash: &str) -> Result<Option<String>, sqlx::Error> {
    let result = query_as!(
        FileMetadata,
        r"
        SELECT mimetype
        FROM files
        WHERE hash_filename = ?
        ",
        hash
    )
    .fetch_optional(db)
    .await?;

    Ok(result.map(|meta| meta.mimetype))
}

#[cfg(test)]
mod tests {
    use super::get_file_mimetype;
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
    async fn returns_mimetype_for_known_hash() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO files (hash_filename, mimetype, file_size_bytes) VALUES (?, ?, ?)",
        )
        .bind("abc123")
        .bind("image/png")
        .bind(42_i64)
        .execute(&db)
        .await
        .expect("file fixture should insert");

        let mimetype = get_file_mimetype(&db, "abc123")
            .await
            .expect("metadata query should succeed");

        assert_eq!(mimetype.as_deref(), Some("image/png"));
    }

    #[tokio::test]
    async fn returns_none_for_unknown_hash() {
        let db = test_db().await;

        let mimetype = get_file_mimetype(&db, "missing")
            .await
            .expect("metadata query should succeed");

        assert_eq!(mimetype, None);
    }
}
