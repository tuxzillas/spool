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
    let mut mimetype: Option<String> = None;
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

    if let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart body"))?
    {
        if field.name() != Some("file") {
            return Err((
                StatusCode::BAD_REQUEST,
                "please include a file header",
                "request did not include a file header",
            )
                .into());
        }

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

        mimetype = sniffed
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
    }

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
        StatusCode::OK,
        Json(format!(r#"{}/{}"#, &state.config.address, hash_hex)), // this will be replaced with config.domain in the future
    ))
}
