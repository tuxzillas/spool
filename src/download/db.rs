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
