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
