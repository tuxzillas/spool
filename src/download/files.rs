use axum::{extract::Multipart, http::StatusCode};
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::io::Write;
use tokio::sync::mpsc;

use crate::State;
use crate::download::db::insert_file;
use crate::types::ErrorStatus;

// i think my next best step here is to filter out say..
// someone uploading two files at once!
// cause i think this just merges two files into one blob.
// which.. isn't helpful.
// also need to figure out what happens when two people upload at the same time

pub async fn download(
    axum::extract::State(state): axum::extract::State<State>,
    mut multipart: Multipart,
) -> Result<StatusCode, ErrorStatus> {
    let mut file_size_bytes: u64 = 0;
    let mut detected_mimetype: Option<String> = None;
    let mut multipart_mimetype: Option<String> = None;

    let (task, mut trec) = mpsc::channel::<Bytes>(8);

    let write = tokio::task::spawn_blocking(move || {
        let mut hasher = Sha256::new();
        let mut file = tempfile::Builder::new()
            .prefix(".temp-spool-")
            .tempfile_in("/home/tuxzilla/Projects/spool-storage") // not configurable
            .map_err(|_| std::io::Error::other("could not create temp file"))?;

        while let Some(chunk) = trec.blocking_recv() {
            file.write_all(&chunk)?;
            hasher.update(&chunk);
            file_size_bytes = file_size_bytes
                .checked_add(chunk.len() as u64)
                .ok_or(std::io::Error::other("file size too large"))?;
        }

        let hash_hex = hex::encode(hasher.finalize());

        Ok((file, hash_hex, file_size_bytes))
    });

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        if multipart_mimetype.is_none() {
            multipart_mimetype = field.content_type().map(str::to_owned);
        }

        while let Some(chunk) = field.chunk().await.map_err(|_| StatusCode::BAD_REQUEST)? {
            if detected_mimetype.is_none() {
                detected_mimetype = infer::get(&chunk).map(|kind| kind.mime_type().to_owned());
            }

            task.send(chunk)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    drop(task);

    let result = write
        .await
        .ok()
        .and_then(|inner: Result<_, std::io::Error>| inner.ok());

    let (file, hash_hex, file_size_bytes) = if let Some((file, hash_hex, file_size_bytes)) = result
    {
        (file, hash_hex, file_size_bytes)
    } else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR.into());
    };

    // needs to be configurable!
    let final_path = format!("/home/tuxzilla/Projects/spool-storage/{hash_hex}");

    let mimetype = detected_mimetype
        .or(multipart_mimetype)
        .unwrap_or_else(|| "application/octet-stream".to_owned());

    // this is obviously incredibly stupid
    // needs to be a max per config (such as 1gb, etc)
    let file_size_bytes =
        i64::try_from(file_size_bytes).map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;

    let result = tokio::task::spawn_blocking(move || file.persist_noclobber(final_path)).await?;
    // need to filter errrors!
    // match statements?
    if let Err(e) = result {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, e).into());
    }

    // moved this to prevent file metadata being added to the DB, than being written to disk
    let result = insert_file(&state.db, &hash_hex, &mimetype, file_size_bytes).await;
    if let Err(e) = result {
        return Err(e.into());
    }

    Ok(StatusCode::CREATED)
}
