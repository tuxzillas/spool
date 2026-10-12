use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use {std::io::Write, std::path::Path};

use crate::SpoolState;
use crate::download::db::insert_file;
use crate::types::ErrorStatus;

pub async fn download(
    State(state): State<SpoolState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<String>), ErrorStatus> {
    let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart body"))?
    else {
        return Err((StatusCode::BAD_REQUEST, "missing file field").into());
    };

    if field.name() != Some("file") {
        return Err((
            StatusCode::BAD_REQUEST,
            "please include a file header",
            "request did not include a file header",
        )
            .into());
    }

    let max_bytes = state.config.max_file_size_bytes as u64;
    let (task, mut trec) = mpsc::channel::<Bytes>(8);

    let write = tokio::task::spawn_blocking(move || -> Result<_, ErrorStatus> {
        let mut fsize_bytes: u64 = 0;
        let mut hasher = Sha256::new();
        let mut file = tempfile::Builder::new()
            .prefix(".temp-spool-")
            .tempfile_in(&state.config.storage_path)?;

        while let Some(chunk) = trec.blocking_recv() {
            fsize_bytes = fsize_bytes
                .checked_add(chunk.len() as u64)
                .ok_or((StatusCode::PAYLOAD_TOO_LARGE, "file too large"))?;

            if fsize_bytes > max_bytes {
                drop(file);
                return Err((StatusCode::PAYLOAD_TOO_LARGE, "file too large").into());
            }

            file.write_all(&chunk)?;
            hasher.update(&chunk);
        }

        let hash_hex = hex::encode(hasher.finalize());

        file.persist_noclobber(Path::new(&state.config.storage_path).join(&hash_hex))?;
        Ok((hash_hex, fsize_bytes))
    });

    let mut sniffed: Option<&str> = None;
    let mut first_chunk = true;

    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "failed to read field chunks"))?
    {
        if first_chunk {
            first_chunk = false;
            sniffed = infer::get(&chunk).map(|kind| kind.mime_type());
        }
        task.send(chunk)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to write file"))?;
    }

    let mimetype = sniffed
        .or_else(|| {
            field
                .content_type()
                .filter(|ctype| *ctype != "application/octet-stream")
        })
        .or_else(|| {
            field
                .file_name()
                .and_then(|name| mime_guess::from_path(name).first_raw())
        })
        .map(str::to_owned);

    drop(task);

    let (hash_hex, fsize_bytes) = write.await??;

    insert_file(
        &state.db,
        &hash_hex,
        &mimetype.unwrap_or_else(|| "application/octet-stream".to_owned()),
        fsize_bytes.cast_signed(),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(format!(r"{}/{}", state.config.address, hash_hex)),
    ))
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::expect_used)]
#[cfg(test)]
mod tests {
    use super::download;
    use crate::{Config, SpoolState};
    use axum::{
        Json,
        body::Body,
        extract::{FromRequest, Multipart, State},
        http::{Request, StatusCode},
        response::IntoResponse,
    };
    use sha2::Digest;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::TempDir;

    async fn test_state(storage: &TempDir, max_file_size_bytes: usize) -> SpoolState {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database should connect");
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("migrations should succeed");
        SpoolState {
            db,
            config: Config {
                address: "http://localhost".to_owned(),
                max_file_size_bytes,
                storage_path: storage.path().to_string_lossy().into_owned(),
            },
        }
    }

    async fn multipart(body: &str) -> Multipart {
        let request = Request::builder()
            .header("content-type", "multipart/form-data; boundary=boundary")
            .body(Body::from(body.to_owned()))
            .expect("request should build");
        Multipart::from_request(request, &())
            .await
            .expect("multipart request should parse")
    }

    fn file_part(name: &str, headers: &str, content: &str) -> String {
        format!(
            "--boundary\r\nContent-Disposition: form-data; name=\"{name}\"{headers}\r\n\r\n{content}\r\n--boundary--\r\n"
        )
    }

    #[tokio::test]
    async fn stores_file_with_content_hash_size_and_detected_mime_type() {
        let storage = TempDir::new().expect("temporary storage should be created");
        let state = test_state(&storage, 1024).await;
        let data = b"hello upload";
        let body = format!(
            "--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\n{}\r\n--boundary--\r\n",
            String::from_utf8_lossy(data)
        );

        let (status, Json(url)) = download(State(state.clone()), multipart(&body).await)
            .await
            .expect("upload should succeed");
        let hash = hex::encode(sha2::Sha256::digest(data));
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(url, format!("http://localhost/{hash}"));
        assert_eq!(
            std::fs::read(storage.path().join(&hash)).expect("file should exist"),
            data
        );
        let row: (String, i64) =
            sqlx::query_as("SELECT mimetype, file_size_bytes FROM files WHERE hash_filename = ?")
                .bind(&hash)
                .fetch_one(&state.db)
                .await
                .expect("metadata should be inserted");
        assert_eq!(row, ("text/plain".to_owned(), data.len() as i64));
    }

    #[tokio::test]
    async fn accepts_empty_file_and_records_zero_bytes() {
        let storage = TempDir::new().expect("temporary storage should be created");
        let state = test_state(&storage, 0).await;
        let body = file_part("file", "; filename=\"empty.bin\"", "");
        let result = download(State(state.clone()), multipart(&body).await).await;
        assert!(result.is_ok(), "empty files are within a zero-byte limit");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE file_size_bytes = 0")
            .fetch_one(&state.db)
            .await
            .expect("metadata query should succeed");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn rejects_file_larger_than_limit_without_persisting_it() {
        let storage = TempDir::new().expect("temporary storage should be created");
        let state = test_state(&storage, 3).await;
        let body = file_part("file", "", "four");
        let response = download(State(state.clone()), multipart(&body).await)
            .await
            .expect_err("oversized upload should fail")
            .into_response();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            std::fs::read_dir(storage.path())
                .expect("storage should exist")
                .count(),
            0
        );
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
            .fetch_one(&state.db)
            .await
            .expect("metadata query should succeed");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn rejects_first_field_that_is_not_named_file() {
        let storage = TempDir::new().expect("temporary storage should be created");
        let state = test_state(&storage, 100).await;
        let body = file_part("other", "", "data");
        let response = download(State(state.clone()), multipart(&body).await)
            .await
            .expect_err("wrong field should fail")
            .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            std::fs::read_dir(storage.path())
                .expect("storage should exist")
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn rejects_multipart_body_without_file_field() {
        let storage = TempDir::new().expect("temporary storage should be created");
        let state = test_state(&storage, 100).await;
        let response = download(State(state.clone()), multipart("--boundary--\r\n").await)
            .await
            .expect_err("missing file field should fail")
            .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
            .fetch_one(&state.db)
            .await
            .expect("metadata query should succeed");
        assert_eq!(count, 0);
        assert_eq!(
            std::fs::read_dir(storage.path())
                .expect("storage should exist")
                .count(),
            0
        );
    }
}
