use axum::extract::Path;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderName, StatusCode, header, header::CONTENT_DISPOSITION},
    response::IntoResponse,
};
use std::path::PathBuf;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::{SpoolState, types::ErrorStatus};

use crate::serve::db::get_file_mimetype;

const DOWNLOAD_CHUNK_SIZE: usize = 512 * 1024;

pub async fn serve(
    State(state): State<SpoolState>,
    Path(file_hash): Path<String>,
) -> Result<impl IntoResponse, ErrorStatus> {
    if file_hash.len() != 64 || !file_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((StatusCode::BAD_REQUEST, "Invalid hash").into());
    }
    let Ok(file) = File::open(PathBuf::from(&state.config.storage_path).join(&file_hash)).await
    else {
        return Err((StatusCode::NOT_FOUND, "File not found").into());
    };
    let Some(mimetype) = get_file_mimetype(&state.db, &file_hash).await? else {
        return Err((StatusCode::NOT_FOUND, "File not found").into());
    };

    let stream = ReaderStream::with_capacity(file, DOWNLOAD_CHUNK_SIZE);
    let body = Body::from_stream(stream);

    let headers = [
        (header::CONTENT_TYPE, mimetype),
        (
            HeaderName::from_static("x-content-type-options"),
            "nosniff".parse().unwrap(),
        ),
        (CONTENT_DISPOSITION, "attachment".parse().unwrap()),
    ];

    Ok((headers, body))
}

#[allow(clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::serve;
    use crate::{Config, SpoolState};
    use axum::{
        body::to_bytes,
        extract::{Path, State},
        http::{StatusCode, header},
        response::IntoResponse,
    };
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::TempDir;

    #[tokio::test]
    async fn serves_untrusted_html_as_attachment_with_security_headers() {
        let storage = TempDir::new().expect("storage directory should be created");
        let hash = "a".repeat(64);
        tokio::fs::write(storage.path().join(&hash), "<script>alert(1)</script>")
            .await
            .expect("fixture file should be written");
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database should connect");
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("migrations should succeed");
        sqlx::query(
            "INSERT INTO files (hash_filename, mimetype, file_size_bytes) VALUES (?, ?, ?)",
        )
        .bind(&hash)
        .bind("text/html")
        .bind(26_i64)
        .execute(&db)
        .await
        .expect("HTML metadata should be inserted");

        let response = serve(
            State(SpoolState {
                db,
                config: Config {
                    address: "http://localhost".to_owned(),
                    max_file_size_bytes: 1024,
                    storage_path: storage.path().to_string_lossy().into_owned(),
                },
            }),
            Path(hash.clone()),
        )
        .await
        .expect("known file should be served")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "text/html");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(
            response.headers()[header::CONTENT_DISPOSITION],
            "attachment"
        );
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("response body should read"),
            "<script>alert(1)</script>"
        );
    }
}
