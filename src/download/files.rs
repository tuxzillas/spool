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
    let mut file_size_bytes: u64 = 0;
    let mut detected_mimetype: Option<String> = None;
    let mut multipart_mimetype: Option<String> = None;

    let (task, mut trec) = mpsc::channel::<Bytes>(8);
    let max_file_size_bytes = state.config.max_file_size_bytes as u64;

    let write = tokio::task::spawn_blocking(move || -> Result<_, ErrorStatus> {
        let mut hasher = Sha256::new();
        let mut file = tempfile::Builder::new()
            .prefix(".temp-spool-")
            .tempfile_in(&state.config.storage_path)?;

        while let Some(chunk) = trec.blocking_recv() {
            file_size_bytes = file_size_bytes
                .checked_add(chunk.len() as u64)
                .ok_or((StatusCode::PAYLOAD_TOO_LARGE, "file too large"))?;

            if file_size_bytes > max_file_size_bytes {
                drop(file);
                return Err((StatusCode::PAYLOAD_TOO_LARGE, "file too large").into());
            }

            file.write_all(&chunk)?;
            hasher.update(&chunk);
        }

        let hash_hex = hex::encode(hasher.finalize());

        Ok((file, hash_hex, file_size_bytes, state.config.storage_path))
    });

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart body"))?
    {
        if field.name() != Some("file") {
            return Err((StatusCode::BAD_REQUEST, "expected a field named 'file'").into());
        }

        multipart_mimetype = field.content_type().map(str::to_owned);

        let mut field_size_bytes: u64 = 0;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart body"))?
        {
            field_size_bytes = field_size_bytes
                .checked_add(chunk.len() as u64)
                .filter(|&size| size <= max_file_size_bytes)
                .ok_or((StatusCode::PAYLOAD_TOO_LARGE, "file too large"))?;

            if detected_mimetype.is_none() {
                detected_mimetype = infer::get(&chunk).map(|kind| kind.mime_type().to_owned());
            }

            task.send(chunk)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "upload pipeline closed"))?;
        }
    }

    drop(task);

    let (file, hash_hex, file_size_bytes, storage_path) = write.await??;

    let mimetype = detected_mimetype
        .or(multipart_mimetype)
        .unwrap_or_else(|| "application/octet-stream".to_owned());

    let file_size_bytes = i64::try_from(file_size_bytes) // this is ass
        .map_err(|_| (StatusCode::PAYLOAD_TOO_LARGE, "file too large"))?;

    // the more I read this the more I think "why the hell are we spawning a tokio task for basically a file rename??"
    // i need to do some tests on if doing this as sync harms performance any
    // wait, couldn't i just.. put this in the end of the loop above??
    let hash_hex = tokio::task::spawn_blocking(move || -> Result<String, ErrorStatus> {
        file.persist_noclobber(Path::new(&storage_path).join(&hash_hex))?;

        Ok(hash_hex)
    })
    .await??;

    insert_file(&state.db, &hash_hex, &mimetype, file_size_bytes).await?;

    Ok(StatusCode::CREATED)
}
