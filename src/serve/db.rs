use sqlx::AnyPool;
use sqlx::{FromRow, query_as};

#[derive(Debug, FromRow)]
struct FileMetadata {
    mimetype: String,
}

pub async fn get_file_mimetype(db: &AnyPool, hash: &str) -> Result<Option<String>, sqlx::Error> {
    let result = query_as::<_, FileMetadata>(
        r#"
        SELECT mimetype
        FROM files
        WHERE hash_filename = ?
        "#,
    )
    .bind(hash)
    .fetch_optional(db)
    .await?;

    Ok(result.map(|meta| meta.mimetype))
}
