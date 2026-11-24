use crate::Database;
use sqlx::{PgPool, SqlitePool};

impl TryFrom<Database> for SqlitePool {
    type Error = Database;

    fn try_from(database: Database) -> Result<Self, Self::Error> {
        match database {
            Database::Sqlite(pool) => Ok(pool),
            database => Err(database),
        }
    }
}

impl TryFrom<Database> for PgPool {
    type Error = Database;

    fn try_from(database: Database) -> Result<Self, Self::Error> {
        match database {
            Database::Postgres(pool) => Ok(pool),
            database => Err(database),
        }
    }
}
