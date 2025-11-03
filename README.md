# s-curve

A modular Axum-based backend for project and task management. It layers JWT authentication, SQLite persistence (SQLx), and first-class OpenAPI docs, and ships with a companion CLI to manage migrations.

## Features

- Axum 0.7 HTTP stack with layered middleware for CORS, tracing, and shared state.
- SQLite via SQLx with UUID primary keys and `deleted_at` soft deletes.
- Argon2 password hashing and JWT tokens backed by an env-configurable secret.
- Projects and tasks domain APIs secured by authenticated user context.
- CLI utilities for generating and running sqlx migrations.
- Swagger UI served at `/docs` using `utoipa` and `utoipa-swagger-ui`.

## Getting Started

```bash
# clone repo (example)
# git clone <repo> && cd s-curve

# ensure toolchain
rustup target add x86_64-unknown-linux-gnu

# install sqlx CLI (optional, useful for debugging)
cargo install sqlx-cli --no-default-features --features sqlite

# configure environment
cp .env .env.local  # or edit .env directly
```

Set at minimum:

```
DATABASE_URL=sqlite://s-curve.db
JWT_SECRET=replace-me
JWT_EXP_HOURS=24
APP_PORT=8000
```

## Database Migrations

The embedded CLI handles migration lifecycle:

```bash
# create a new timestamped migration file
cargo run --bin cli -- make-migration add_labels_to_tasks

# apply pending migrations
cargo run --bin cli -- migrate-run

# show database vs. disk migrations
cargo run --bin cli -- migrate-status

# undo the last migration
cargo run --bin cli -- migrate-rollback
```

Generated files live under `migrations/` and are executed against the `DATABASE_URL` configured in your env.

### Migrations (Docker / container notes)

When running the CLI from inside a container (for example the `rust-service` developer container), pay attention to the following:

- The CLI reads `DATABASE_URL` from your environment. If you use a mounted workspace, ensure the container has an accessible `.env` file or export `DATABASE_URL` into the container environment.
- SQLite requires the process have write access to the directory containing the database file. If the DB path is `sqlite:///apps/scurve-be/scurve.sqlite`, make sure the directory and file are writable by the container user (or create the file ahead of time with the right owner/permissions).

Docker-friendly examples (run inside the container or via `docker exec`):

```bash
# apply pending migrations inside the container (uses the crate's Cargo.toml path)
docker exec -it rust-service \
  cargo run --manifest-path scurve-be/Cargo.toml \
  --target-dir scurve-be/target --release --bin cli -- migrate-run

# view migration status
docker exec -it rust-service \
  cargo run --manifest-path scurve-be/Cargo.toml \
  --target-dir scurve-be/target --release --bin cli -- migrate-status

# if the DB file doesn't exist or permissions are wrong, create it and set ownership
docker exec -it rust-service sh -lc 'touch /apps/scurve-be/scurve.sqlite && chown $USER:$USER /apps/scurve-be/scurve.sqlite && chmod 644 /apps/scurve-be/scurve.sqlite'
```

Recommended container entrypoint snippet (developer convenience): create the DB file and ensure ownership before starting the service. Adapt the user / owner to your runtime user inside the container.

```sh
# entrypoint snippet (POSIX sh)
set -e
DB_PATH="/apps/scurve-be/scurve.sqlite"
if [ ! -f "$DB_PATH" ]; then
  mkdir -p "$(dirname "$DB_PATH")"
  touch "$DB_PATH"
  chown 1000:1000 "$DB_PATH" || true
  chmod 644 "$DB_PATH" || true
fi
exec "$@"
```

If you regularly mount the workspace from the host, make sure the host-side file ownership/UID mapping is compatible with the container user or the entrypoint creates the DB with the correct ownership.

## Running the API

```bash
# start HTTP server (default-run = s-curve binary)
cargo run
# or release profile
cargo run --release
```

The server listens on `0.0.0.0:<APP_PORT>` (default `8000`). Visit `http://localhost:<APP_PORT>/docs` for Swagger UI.

## Available Endpoints

| Method | Path | Auth | Purpose |
| ------ | ---- | ---- | ------- |
| POST | `/auth/register` | ❌ | Register a user |
| POST | `/auth/login` | ❌ | Obtain JWT |
| GET | `/auth/me` | ✅ | Current user profile |
| POST | `/auth/logout` | ✅ | Stateless logout acknowledgement |
| GET/POST | `/projects` | ✅ | List / create projects |
| GET/PUT/DELETE | `/projects/{id}` | ✅ | Read / update / soft delete project |
| GET/POST | `/tasks` | ✅ | List / create tasks |
| PUT/DELETE | `/tasks/{id}` | ✅ | Update / soft delete task |

Requests requiring auth expect an `Authorization: Bearer <token>` header. Register then log in to retrieve a token.

## Development Notes

- Soft deletes are implemented by setting `deleted_at`; queries filter out non-null values.
- IDs are generated with `Uuid::new_v4()` and timestamps use `chrono::Utc::now()`.
- The project integrates `tower-http` tracing; set `RUST_LOG=debug` to expand logs.
- Tests are not included; consider wiring integration tests with an ephemeral SQLite database for coverage.

## Docker Usage

When using the provided container (e.g., `rust-service`), supply explicit paths to keep build artifacts inside the workspace:

```bash
docker exec -it rust-service env RUST_BACKTRACE=1 CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem cargo run \
  --manifest-path scurve-be/Cargo.toml \
  --target-dir scurve-be/target \
  --release
```

## Using Swagger UI (Interactive API docs)

The project exposes a Swagger UI at `/docs` and a machine-readable OpenAPI JSON at
`/api-docs/openapi.json`. When the server is running (default port: `8000`, or
`APP_PORT`), open the docs in your browser:

  http://localhost:8800/docs

Notes on how to use the UI effectively:

- Authorize (JWT Bearer):
  1. Click the "Authorize" button in the top-right of Swagger UI.
  2. Enter your token prefixed with `Bearer `, e.g.:

     Bearer eyJhbGciOiJIUzI1Ni...

  3. Click "Authorize". Swagger UI will persist the token (if configured) and
     include it as the `Authorization` header for any endpoints that require
     authentication.

- Try it out (request examples):
  - Many endpoints include example request bodies and response schemas. Click
    an operation, then click "Try it out" to enable the request editor.
  - The request body will be pre-filled with a sensible example (where
    available). Edit it if you need custom payloads, then click "Execute".

- Inspect server responses:
  - Swagger UI shows the response status, headers, and JSON body. For endpoints
    that return example responses, the example will be shown even when the
    backend is not exercised.

- When the UI reports a "duplicated mapping key" or "invalid version field":
  - This repository includes runtime sanitization that merges and normalizes
    path/method keys before serving the OpenAPI JSON. If you still see parser
    errors, try fetching the raw OpenAPI JSON and inspect it:

    ```bash
    curl -sS http://localhost:8800/api-docs/openapi.json | jq . > /tmp/openapi.json
    jq 'keys' /tmp/openapi.json
    ```

  - If the `openapi` top-level key is missing, ensure your build is running the
    latest code and that `APP_PORT` is set correctly. Rebuild and restart the
    container if necessary.

- Development / Docker notes:
  - To run locally (not in Docker):

    ```bash
    APP_PORT=8800 cargo run --release
    ```

  - To run inside the `rust-service` container (developer setup used here):

    ```bash
    docker exec -it rust-service \
      env RUST_BACKTRACE=1 CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
      cargo run --manifest-path scurve-be/Cargo.toml \
      --target-dir scurve-be/target --release
    ```

  - If you need to debug the OpenAPI output server-side, fetch the JSON from
    inside the container and inspect it with Python's json parser (which will
    detect duplicate object keys when using an object_pairs_hook):

    ```bash
    docker exec -it rust-service bash -lc "curl -sS http://localhost:8800/api-docs/openapi.json > /tmp/openapi.json && python3 -c 'import json,sys; print(json.loads(open("/tmp/openapi.json").read()))'"
    ```

What to expect
- The Swagger UI should display the `Auth`, `Projects`, and `Tasks` tag groups
  and allow you to call the endpoints using the provided examples.
- Endpoints that require auth will show a lock icon; after Authorize they will
  send `Authorization: Bearer <token>` on Try-it-out requests.

## HTTP/2 and HTTPS (TLS) support

This service prefers TLS (HTTPS) for browser compatibility and will negotiate
HTTP/2 automatically via ALPN when TLS is enabled. Browsers require TLS for
HTTP/2, so the project uses `axum-server` + Rustls to provide a secure server
that supports ALPN/HTTP2 negotiation.

How to enable TLS

- Provide PEM files via environment variables before starting the server:

  CERT_PATH=/path/to/cert.pem
  KEY_PATH=/path/to/key.pem

  Then start normally (the server will detect these env vars and start in
  TLS mode):

  APP_PORT=8800 CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem cargo run --release

- For local development you can generate a temporary self-signed certificate
  (the repository includes optional helpers to generate one). In browsers you
  will need to accept the self-signed certificate (or install it into your
  trust store) to avoid certificate warnings.

What changes in the OpenAPI docs

- When TLS is active (CERT_PATH/KEY_PATH set), the OpenAPI `servers` array is
  updated to include `https://localhost:<PORT>` so the Swagger UI "Try it out"
  uses HTTPS URLs by default. If TLS is not enabled, `http://localhost:<PORT>` is used.

Testing TLS and HTTP/2

- Quick curl test (accepts self-signed certs with `-k`):

  curl -vk --http2 https://localhost:8800/docs

  The `--http2` flag ensures curl attempts HTTP/2; `-k` is only needed for
  self-signed certificates during development.

- Use the browser DevTools Network panel to confirm the negotiated protocol
  (look for "h2" or "http/2" in the Protocol/Version column). If the
  browser refuses to connect because of an untrusted cert, either accept the
  warning for testing or install the certificate into your local trust store.

Notes and troubleshooting

- Browsers will not use cleartext h2c (HTTP/2 prior knowledge) for remote sites
  — TLS + ALPN is the standard path for production-grade HTTP/2.
- If you see the Swagger UI still calling `http://...` endpoints after enabling
  TLS, confirm the server was restarted with `CERT_PATH`/`KEY_PATH` set and
  check the `servers` field in `/api-docs/openapi.json`.
- For automated tests or non-browser clients you can use `curl --http2-prior-knowledge`
  against an h2c server, but note that this project uses TLS/ALPN for browser
  compatibility and does not enable h2c by default.

If anything in your environment prevents the server from reading your cert/key
files (permission issues, wrong paths), the application will fall back to
plaintext HTTP on the configured `APP_PORT` unless you explicitly handle
failures in your deployment scripts.
