use crate::Database;

impl Database {
    pub async fn insert_file(
        &self,
        hash: &str,
        mimetype: &str,
        file_size_bytes: i64,
    ) -> Result<bool, sqlx::Error> {
        match self {
            Database::Sqlite(pool) => {
                Ok(spool_sqlite::insert_file(pool, hash, mimetype, file_size_bytes).await?)
            }
            Database::Postgres(pool) => {
                Ok(spool_postgres::insert_file(pool, hash, mimetype, file_size_bytes).await?)
            }
        }
    }

    pub async fn get_file_metadata(&self, hash: &str) -> Result<Option<String>, sqlx::Error> {
        match self {
            Database::Sqlite(pool) => Ok(spool_sqlite::get_file_metadata(&pool, hash).await?),
            Database::Postgres(pool) => Ok(spool_postgres::get_file_metadata(&pool, hash).await?),
        }
    }
}
