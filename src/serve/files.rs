use axum::extract::Path;
use axum::{
    body::Body,
    http::{StatusCode, header},
    response::IntoResponse,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::State;

use crate::serve::db::get_file_mimetype;

pub async fn serve(
    axum::extract::State(state): axum::extract::State<State>,
    Path(file_hash): Path<String>,
) -> impl IntoResponse {
    let file = match File::open(format!("/home/tuxzilla/Projects/spool-storage/{file_hash}")).await // W hardcoded path
    {
        Ok(file) => file,
        Err(_) => return Err((StatusCode::NOT_FOUND, "File not found")),
    };

    let Some(mimetype) = get_file_mimetype(&state.db, &file_hash).await.unwrap() else {
        return Err((StatusCode::NOT_FOUND, "File not found"));
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let headers = [(header::CONTENT_TYPE, mimetype)];

    Ok((headers, body))
}
