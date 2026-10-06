use axum::{
    body::Body,
    http::{StatusCode, header},
    response::IntoResponse,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use axum::extract::Path;

pub async fn send_image(Path(file_hash): Path<String>) -> impl IntoResponse {
    let file = match File::open(format!("/home/tuxzilla/Projects/spool-storage/{file_hash}")).await // W hardcoded path
    {
        Ok(file) => file,
        Err(_) => return Err((StatusCode::NOT_FOUND, "File not found")),
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let headers = [(header::CONTENT_TYPE, "text/html; charset=utf-8")];

    Ok((headers, body))
}
