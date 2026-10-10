use axum::extract::Path;
use axum::{
    body::Body,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
};
use std::path::PathBuf;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::{SpoolState, types::ErrorStatus};

use crate::serve::db::get_file_mimetype;

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

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let headers = [(header::CONTENT_TYPE, mimetype)];

    Ok((headers, body))
}
