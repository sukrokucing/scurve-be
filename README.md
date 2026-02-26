# s-curve

Modular Axum backend for project/task management with JWT auth, SQLite (SQLx), RBAC, and OpenAPI/Swagger.

## What You Get

- Axum 0.7 API with tracing, CORS, and rate limiting
- SQLite persistence with SQLx migrations
- JWT auth + Argon2 password hashing
- RBAC with route-permission mapping
- OpenAPI at `/api-docs/openapi.json` and Swagger UI at `/docs`
- Migration CLI (`cargo run --bin cli -- ...`)

## Quick Start (Local)

```bash
# from repository root
# create .env once (if you don't have it yet)
cp .env.example .env

# run API (applies migrations on startup)
cargo run --release
```

Open:

- `http://localhost:<APP_PORT>/docs` (HTTP mode)
- `https://localhost:<APP_PORT>/docs` (when `CERT_PATH` and `KEY_PATH` are set)

Notes:

- Code fallback default is `APP_PORT=8000`.
- This repository's `.env` currently sets `APP_PORT=8800`.

## Quick Start (Docker: `rust-service`)

The `rust-service` container is usually idle (`tail -f /dev/null`), so run commands via `docker exec`.

```bash
# 1) ensure db file exists and is writable
docker exec rust-service sh -lc 'touch /apps/scurve-be/scurve.sqlite && chmod 664 /apps/scurve-be/scurve.sqlite'

# 2) run migrations
docker exec rust-service cargo run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release --bin cli -- migrate-run

# 3) start API (TLS enabled with mounted certs)
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  cargo run --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --release
```

Health check:

```bash
curl -sk https://localhost:8800/api/health
```

Important:

- Prefer `docker exec rust-service cargo ...` over `docker exec ... sh -lc 'cargo ...'`.
- In this container, `sh -lc` may not include Cargo in `PATH`.

## Configuration

Minimum required environment variables:

```env
DATABASE_URL=sqlite://scurve.sqlite
JWT_SECRET=replace-me
JWT_EXP_HOURS=24
APP_PORT=8800
```

Container default used in this repo:

```env
DATABASE_URL=sqlite:///apps/scurve-be/scurve.sqlite
```

Common optional variables:

- `CERT_PATH`, `KEY_PATH`: enable TLS (and browser HTTP/2 via ALPN)
- `AUTHZ_MODE`: `off` | `advisory` | `strict`
- `SHOW_ERRORS`: include debug detail in error payloads when `true`/`1`
- `AUTH_RATE_PER_SECOND`, `AUTH_BURST_SIZE`
- `GLOBAL_RATE_PER_SECOND`, `GLOBAL_BURST_SIZE`

## Migrations CLI

```bash
# create migration template
cargo run --bin cli -- make-migration add_labels_to_tasks

# apply pending migrations
cargo run --bin cli -- migrate-run

# compare applied vs pending
cargo run --bin cli -- migrate-status

# rollback last migration
cargo run --bin cli -- migrate-rollback
```

Migrations are stored in `migrations/`.

## API Docs

- Swagger UI: `/docs`
- OpenAPI JSON: `/api-docs/openapi.json`

Regenerate committed OpenAPI files:

```bash
# canonical repo snapshot
cargo run --bin dump_openapi -- --port 8000 --out openapi.json

# local/live-style snapshot (matches current .env APP_PORT)
cargo run --bin dump_openapi -- --port 8800 --out openapi-live.json
```

Auth flow in Swagger:

1. `POST /auth/register` or `POST /auth/login`
2. Click **Authorize**
3. Paste `Bearer <token>`
4. Execute protected endpoints

## Endpoint Snapshot

| Method | Path | Auth | Purpose |
| ------ | ---- | ---- | ------- |
| POST | `/auth/register` | No | Register user |
| POST | `/auth/login` | No | Login and get JWT |
| GET | `/auth/me` | Yes | Current user |
| POST | `/auth/logout` | Yes | Stateless logout acknowledgement |
| POST | `/auth/forgot-password` | No | Request reset token |
| POST | `/auth/reset-password` | No | Reset password |
| GET/POST | `/projects` | Yes | List/create projects |
| GET/PUT/DELETE | `/projects/{id}` | Yes | Read/update/delete project |
| GET/POST | `/projects/{project_id}/tasks` | Yes | List/create tasks |
| PUT/DELETE | `/projects/{project_id}/tasks/{id}` | Yes | Update/delete task |
| GET/POST | `/projects/{project_id}/tasks/{task_id}/progress` | Yes | List/create progress |
| PUT/DELETE | `/projects/{project_id}/tasks/{task_id}/progress/{id}` | Yes | Update/delete progress |
| GET/POST/DELETE | `/rbac/...` | Yes | RBAC administration |

## Development & Tests

Test safety model:

- Integration tests clone `scurve.sqlite` into a per-test temp file.
- Each cloned DB is cleaned before use (mutable tables truncated).
- Temp DB files are deleted automatically after each test.
- Your original `scurve.sqlite` is not modified by test runs.

```bash
# run all unit + integration tests
cargo test --tests

# focused test
cargo test --test api_integration
```

Docker test commands (`rust-service`):

```bash
# run all unit + integration tests
docker exec rust-service cargo test \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --tests

# run one test file
docker exec rust-service cargo test \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --test ownership_isolation
```

If `scurve.sqlite` is missing/outdated, refresh it first:

```bash
docker exec rust-service cargo run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release --bin cli -- migrate-run
```

## Troubleshooting

- `failed to run migrations`: run `cargo run --bin cli -- migrate-status` and ensure `DATABASE_URL` points to the intended SQLite file.
- `cargo: not found` in container: do not wrap with `sh -lc`; run Cargo directly via `docker exec rust-service cargo ...`.
- Swagger loads but calls wrong scheme: check whether `CERT_PATH`/`KEY_PATH` are set and restart the server.
