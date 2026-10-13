# AGENTS.md

Guidance for AI coding agents working on **spool**. Read this fully before changing anything.

## What spool is

A small Rust file upload/serving service. Files are stored on disk under their **SHA-256 hash**; metadata lives in SQLite. Pre-0.1, under active development.

- Stack: Rust (edition 2024), Axum, Tokio, sqlx (SQLite default), tower-http tracing, TOML for configs.
- Goal for downloads: push as many bytes as possible. The serve path is performance-sensitive.
- Containerization is planned, not done. No auth yet: the `users` / `api_keys` / `user_files` tables exist but no endpoint uses them.

## Layout

```
src/main.rs            router, config loading, DB connect + migrate, SpoolState
src/types.rs           ErrorStatus (the error type for all handlers)
src/download/          POST /upload  (files.rs = handler, db.rs = queries)
src/serve/             GET /{file_hash}  (files.rs = handler, db.rs = queries)
migrations/            sqlx migrations, run at startup
.sqlx/                 committed sqlx offline query metadata
spool.toml             runtime config (address, max_file_size_bytes, storage_path)
```

**Naming quirk:** the `download` module is the *upload* handler (spool is "downloading" from the client). `serve` is what clients download from. Don't "fix" the names unless asked.

## Endpoints and flow

- `POST /upload`: multipart; the **first field must be named `file`**. Chunks stream over a bounded `mpsc` channel into a `spawn_blocking` writer that writes a `.temp-spool-*` tempfile in `storage_path`, hashes as it goes, enforces `max_file_size_bytes`, then `persist_noclobber`s to `storage_path/<hash>`. Then the metadata row is inserted (`ON CONFLICT DO NOTHING`). Returns `201` with the file URL as a JSON string.
- MIME type precedence: `infer` sniff of the first chunk, then the part's `Content-Type` (unless `application/octet-stream`), then `mime_guess` from the filename, then `application/octet-stream`.
- `GET /{file_hash}`: validate hash, open file, look up mimetype in DB, stream with `ReaderStream` (512 KB chunks).

## Invariants (do not break)

1. **Validate before touching the filesystem.** The serve handler requires exactly 64 ASCII hex chars before building a path. Never join user input into a path without this kind of check (path traversal).
2. **Atomic uploads.** Temp file in the same directory as the destination, then rename. A partial or oversized upload must leave nothing behind in storage or the DB.
3. **Size limit is enforced while streaming**, not after. Don't buffer whole uploads in memory.
4. **Content-addressed storage.** The filename on disk is the SHA-256 hex digest. Identical content maps to one blob and one `files` row; a conflicting insert must not overwrite the original row.
5. **No panics on request paths.** No `unwrap`/`expect`/`panic!`/`todo!` outside tests (these are clippy-linted). Propagate with `?` into `ErrorStatus`.
6. **Don't add blocking or write-side work to the serve path.** Serving should stay: validate, open, look up mimetype, stream.
7. **When delete is implemented:** delete the file on disk before the DB row; return a 500 on filesystem failure rather than panicking or removing the row anyway.

## Error handling

Handlers return `Result<_, ErrorStatus>`. `ErrorStatus` separates `public_message` (sent to the client) from `log_message` (logged via `tracing::error!`).

- Use `(StatusCode, "public message").into()`, or the 3-tuple `(StatusCode, "public", "log detail").into()` when the log needs more context.
- Add a `From<...> for ErrorStatus` impl for new error sources instead of `.map_err` boilerplate where it recurs.
- **Never put internal error text in `public_message`.**

## Database

- Queries use sqlx compile-time macros (`query!`, `query_as!`). SQLite only for now: `AnyPool` doesn't support them, so the `postgres` feature is a stub. Don't half-wire Postgres.
- After adding or changing any macro query, regenerate and **commit** the offline data: with `sqlx-cli` and `DATABASE_URL` pointing at a migrated DB, run `cargo sqlx prepare`. CI builds without a database, so a stale `.sqlx/` breaks it.
- Migrations are ordered by version number. The existing one is `20261209_init_schema.sql`, so new migrations must sort **after** it. Never edit an applied migration; add a new one.
- `*.db`, `*.db-shm`, `*.db-wal` are gitignored. Don't commit databases.

## Code style

- **Chain method calls** rather than introducing throwaway `let` bindings. `PathBuf::from(root).join(hash)` beats binding each step. Bind a value only if it's reused or a name genuinely clarifies it.
- Clippy `pedantic` is on, plus `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable`, `dbg_macro`, `redundant_clone`, `print_stdout`, `print_stderr`. Use `tracing`, never `println!`/`dbg!`.
- Prefer `let ... else` for early returns, `cast_signed()` / `try_from` over bare `as` where it matters.
- Keep handlers thin; put DB access in the module's `db.rs`.
- Comments explain *why*. No dead code, no commented-out blocks.
- Follow the nearest existing code. Don't add dependencies without saying why.

## Testing

- Tests live in `#[cfg(test)] mod tests` beside the code. Test modules use `#[allow(clippy::expect_used)]`, so `expect` is fine there and nowhere else.
- DB tests use in-memory SQLite (`max_connections(1)`) and run the real migrations. Storage tests use `tempfile::TempDir`. **Never touch the real `storage_path`.**
- Failure-path tests should assert nothing leaked: no files left in storage, no rows in `files`.
- To check serve-path changes: `scripts/benchmark-serving.sh SOURCE_FILE DOWNLOAD_URL` (verifies SHA-256 and prints throughput).

## Before you say you're done

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test
```

CI runs clippy and `cargo llvm-cov` with `--all-features`, so changes must compile with both the `sqlite` and `postgres` features enabled.

## Working in this repo

- Keep changes small and scoped. Others (human or agent) may be working in parallel; don't reformat or refactor unrelated files.
- If a request is ambiguous or conflicts with an invariant above, stop and ask.
- Mention bugs you notice outside your task in your summary instead of fixing them silently.
- When finished, summarize what changed and why, and flag assumptions.
