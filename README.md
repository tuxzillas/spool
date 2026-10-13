# spool

A small Rust file-upload and serving service. Uploaded files are stored under their SHA-256 hash, with file metadata kept in SQLite. Files can then be retrieved by hash.

## Requirements

- Rust (edition 2024)
- A writable storage directory

## Configuration

At startup, `spool` reads `spool.toml` from the current working directory. Configure the bind address and add a proper storage path there:

```toml
address = "0.0.0.0:3000"
max_file_size_bytes = 1073741824
storage_path = "/path/to/spool-storage/"
```

Create the storage directory before starting the service. The SQLite database (`spool.db`) is created in the working directory and migrations are applied at startup. The default database backend is SQLite.

## Run

From the project directory:

```sh
cargo run
```

(Or `cargo build --release` and run the binary directly.)

The server listens on the configured address (port `3000` by default).

## HTTP endpoints

- `GET /` — basic service greeting.
- `POST /upload` — upload multipart form data. A successful upload returns `201 Created`. The stored file is addressed by its SHA-256 hash.
- `GET /{file_hash}` — retrieve a file by its 64-character SHA-256 hash.

Uploaded files are served with their recorded `Content-Type`, `X-Content-Type-Options: nosniff`, and `Content-Disposition: attachment`. Browsers should download these responses rather than render them as pages. A page hosted elsewhere can still embed supported image files from spool, though directly visiting an image URL also prompts a download.

Uploads are untrusted content. Before exposing spool publicly, serve it from a separate origin that does not share cookies or other credentials with any trusted application. The response headers reduce browser interpretation risks, but deployment should preserve this origin boundary; authentication, quotas, and rate limiting are not implemented.

Example download:

```sh
curl http://localhost:3000/<file_hash> --output downloaded-file
```

## Notes

This project is under development. The schema includes user and API-key tables, but the current HTTP endpoints do not implement authentication or per-user upload management. Review upload size handling and the storage path configuration before exposing the service to untrusted clients.

yes, it will be containerized in the future.
