use axum::{extract::Multipart, extract::State, http::StatusCode};
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
) -> Result<StatusCode, ErrorStatus> {
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

    while let Some(mut field) = multipart
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
        let filename = field.file_name().map(str::to_owned);
        let client_content_type = field.content_type().map(str::to_owned);
        let mut inferred_type: Option<String> = None;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, "failed to read field chunks"))?
        {
            if inferred_type.is_none() {
                if let Some(kind) = infer::get(&chunk) {
                    inferred_type = Some(kind.mime_type().to_owned());
                }
            }
            task.send(chunk)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to write file"))?;
        }

        mimetype = inferred_type
            .or_else(|| client_content_type.filter(|ct| ct != "application/octet-stream"))
            .or_else(|| {
                filename
                    .and_then(|name| mime_guess::from_path(&name).first().map(|m| m.to_string()))
            });
        break;
    }

    drop(task);

    let (hash_hex, fsize_bytes) = write.await??;

    insert_file(
        &state.db,
        &hash_hex,
        &mimetype.unwrap_or_else(|| "application/octet-stream".to_owned()),
        fsize_bytes as i64,
    )
    .await?;

    Ok(StatusCode::CREATED)
}
