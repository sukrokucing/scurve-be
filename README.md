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
# faster compile/startup for daily development
cargo run --profile local-release

# production-like build profile
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
docker exec rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release --bin cli -- migrate-run

# 3) start API (TLS enabled with mounted certs)
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  cargo +1.88.0 run --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --profile local-release

# 4) optional production-like startup
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  cargo +1.88.0 run --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --release
```

Health check:

```bash
curl -sk https://localhost:8800/api/health
```

Important:

- Prefer `docker exec rust-service cargo ...` over `docker exec ... sh -lc 'cargo ...'`.
- In this container, `sh -lc` may not include Cargo in `PATH`.
- Current dependency graph requires Rust 1.88 (`time` crate). Use `cargo +1.88.0 ...` in the container.

### Faster Docker compile/restart

If you restart the API frequently, compile once and run the binary directly:

```bash
# compile once
docker exec rust-service cargo +1.88.0 build \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --profile local-release

# start without invoking Cargo
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  /apps/scurve-be/target/local-release/s-curve
```

Production-like variant:

```bash
docker exec rust-service cargo +1.88.0 build \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release

docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  /apps/scurve-be/target/release/s-curve
```

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
- `AUTHZ_PRINCIPAL_CACHE_MS`: in-memory principal cache TTL in milliseconds (default `0`, disabled)
- `UUID_TEXT_FAST_PATH`: `true|false` fast UUID predicate mode; enable only after UUID canonicalization migration
- `SCURVE_COST_CURRENCY`: fallback ISO currency code for cost metrics (default `USD`)
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
| GET | `/projects/{project_id}/members` | Yes | List active project members + `access_role` + `resource_roles[]` |
| POST | `/projects/{project_id}/members` | Yes | Add/update project member (`access_role_id`, `resource_role_ids[]`) |
| DELETE | `/projects/{project_id}/members/{user_id}` | Yes | Soft-delete project membership |
| GET | `/resource-roles` | Yes | List global resource role catalog |
| POST | `/resource-roles` | Yes | Create global resource role |
| PUT/DELETE | `/resource-roles/{id}` | Yes | Update/soft-delete global resource role |
| GET | `/projects/{project_id}/resource-roles` | Yes | List effective resource roles + project override rates |
| PUT/DELETE | `/projects/{project_id}/resource-roles/{resource_role_id}/rate` | Yes | Upsert/soft-delete project rate override |
| GET/POST | `/projects/{project_id}/tasks` | Yes | List/create tasks |
| DELETE | `/projects/{project_id}/tasks/batch` | Yes | Soft-delete multiple tasks atomically |
| PUT/DELETE | `/projects/{project_id}/tasks/{id}` | Yes | Update/delete task |
| GET | `/projects/{project_id}/tasks/{id}/activity` | Yes | Task activity timeline |
| GET/PUT | `/projects/{project_id}/tasks/{id}/progress-components` | Yes | List/replace weighted progress components for `weighted_components` tasks |
| GET | `/projects/{project_id}/assignees` | Yes | List distinct assignees used in project tasks |
| GET/POST | `/projects/{project_id}/tasks/{task_id}/progress` | Yes | List/create progress |
| PUT/DELETE | `/projects/{project_id}/tasks/{task_id}/progress/{id}` | Yes | Update/delete progress |
| GET/POST | `/projects/{project_id}/tasks/{task_id}/work-logs` | Yes | List/create economic work logs |
| PUT/DELETE | `/projects/{project_id}/tasks/{task_id}/work-logs/{id}` | Yes | Update/soft-delete work log |
| GET | `/tasks/{task_id}/progress` | Yes | Legacy compatibility lookup by task id |
| GET | `/users/me/projects` | Yes | My accessible projects + effective scoped permissions |
| GET/PUT | `/projects/{project_id}/task-health/rules` | Yes | Read/update effective project task-health thresholds |
| GET | `/projects/{id}/dashboard` | Yes | Dashboard payload with metric series (`metric=progress|hours|cost`) plus task summary aggregates |
| GET | `/projects/{id}/s-curve/health` | Yes | S-curve health (`metric=progress|hours|cost`) |
| GET | `/portfolio/s-curve/summary` | Yes | Portfolio-level S-curve summary |
| POST | `/telemetry/events` | Yes | Ingest frontend telemetry batch (idempotent by `event_id`) |
| GET/POST/DELETE | `/rbac/...` | Yes | RBAC administration |

### Task List Query (`GET /projects/{project_id}/tasks`)

Server-side filtering, sorting, and pagination parameters:

| Query Param | Type | Notes |
| --- | --- | --- |
| `q` | string | Case-insensitive title keyword search |
| `status` | string | Single status or comma-separated values (e.g. `todo,done`) |
| `schedule_status` | string | Backend-computed schedule filter: `finished_early`, `overdue`, `on_time`, `not_specified` (comma-separated allowed) |
| `health_status` | string | Derived task health filter: `ahead`, `on_track`, `at_risk`, `critical`, `needs_plan` (comma-separated allowed) |
| `assignee_id` | UUID | Filter by assignee |
| `start_from`, `start_to` | datetime/date | Accepts RFC3339 or `YYYY-MM-DD` |
| `due_from`, `due_to` | datetime/date | Accepts RFC3339 or `YYYY-MM-DD` |
| `sort_by` | string | `start_date`, `due_date`, `created_at`, `updated_at`, `title`, `status`, `progress`, `expected_progress_pct`, `actual_progress_pct`, `variance_pct`, `health_status` |
| `sort_dir` | string | `asc` or `desc` (invalid value returns `400`) |
| `page` | integer | 1-based page number, default `1` |
| `per_page` | integer | Items per page, default `50`, max `100` |

Pagination metadata:

- Response header `X-Total-Count` contains total matching rows (before `LIMIT/OFFSET`).

Legacy compatibility:

- `progress=true` and optional `task_id` are legacy query params on this endpoint.
- Prefer using dedicated progress endpoints for progress payloads.

### Task Description Rules

- `Task`, `TaskCreateRequest`, and `TaskUpdateRequest` now include `description`.
- On create, if `description` is missing/blank, backend auto-fills: `[Quick Add] {title}`.
- On update, blank `description` is rejected with `400`.

### Task Health & Progress Model

- `Task` responses now include:
  - stored planning fields: `progress_method`, `blocked_flag`, `blocked_reason`, `baseline_start_at`, `baseline_end_at`, `task_weight`
  - derived fields: `execution_status`, `expected_progress_pct`, `actual_progress_pct`, `variance_pct`, `health_status`, `expected_progress_source`, `actual_progress_source`
- `progress_method` values:
  - `manual_percent_legacy`
  - `weighted_components`
- `execution_status` values:
  - `not_started`
  - `in_progress`
  - `blocked`
  - `completed`
- `health_status` values:
  - `ahead`
  - `on_track`
  - `at_risk`
  - `critical`
  - `needs_plan`
- `progress` remains on the API as a compatibility mirror.
- Direct `progress` writes are only allowed when `progress_method=manual_percent_legacy`.
- For `weighted_components` tasks, use `GET/PUT /projects/{project_id}/tasks/{id}/progress-components`.
- If a `weighted_components` task receives direct progress writes through task/progress endpoints, backend returns `400`.

### Task Health Rules

- Global defaults are seeded from `USECASE.md`:
  - `critical`: variance `< -25`
  - `at_risk`: variance `>= -25` and `< -10`
  - `on_track`: variance `>= -10` and `< 10`
  - `ahead`: variance `>= 10`
- `GET /projects/{project_id}/task-health/rules` returns the effective rule set for the project.
- `PUT /projects/{project_id}/task-health/rules` stores a project-scoped override.
- `needs_plan` is backend-derived and cannot be configured directly.

### Progress Components

- `PUT /projects/{project_id}/tasks/{id}/progress-components` uses full-set replacement.
- Each component includes:
  - `name`
  - `component_type`
  - `weight`
  - `completion_pct`
  - optional `planned_at`
  - optional `completed_at`
  - optional `sort_order`
- Backend computes `actual_progress_pct` as weighted completion:
  - `SUM(weight * completion_pct) / SUM(weight)`
- If a `weighted_components` task has no active components, `actual_progress_pct` is `null` and `health_status` becomes `needs_plan`.

### Task Completion & Schedule Status

- `Task` responses now include backend-managed `completed_at`, `completed_at_is_backfilled`, and `schedule_status`.
- `completed_at` is set when a task is marked complete by task update or reaches `progress=100` through progress history.
- `completed_at_is_backfilled=true` means the timestamp was reconstructed for legacy data and should not be treated as an explicit completion event.
- Reopening a task clears `completed_at`.
- `schedule_status` is computed by backend as one of:
  - `finished_early`
  - `overdue`
  - `on_time`
  - `not_specified`
- If a task has no due date, `schedule_status` is `not_specified`.

### Dashboard Summary Fields

`GET /projects/{id}/dashboard` now also returns:

- `overall_progress_pct`
- `task_status_counts`
- `workload_distribution`
- `assignment_coverage_pct`
- `due_date_coverage_pct`

Summary rules:

- `overall_progress_pct` is the weighted current actual-progress rollup across non-deleted tasks in the project.
- `task_status_counts` is based on backend-computed schedule status, not raw task status strings.
- `workload_distribution` includes active project members even when they currently have `task_count=0`.
- `workload_distribution` currently measures `task_count`, not capacity or estimated hours.

Progress metric source rules:

- Planned progress prefers `project_plan.planned_progress`.
- If `project_plan` is absent, backend falls back to weighted task `expected_progress_pct`.
- Actual progress comes from weighted task actual-progress rollups.
- If weighted task actuals are unavailable, backend falls back to legacy `task_progress.progress` history.

### Progress vs Work Logs

- Progress endpoints now track `% progress` and optional `note` only.
- `actual_hours` and `actual_cost` were removed from progress request/response schemas.
- Hours/cost economics are captured via task `work-logs` with resource-role + rate snapshots.

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
docker exec rust-service cargo +1.88.0 test \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --tests

# run one test file
docker exec rust-service cargo +1.88.0 test \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --test ownership_isolation
```

One-command validation (`Makefile`):

```bash
# full validation: smoke + fmt + tests + audit + deny
make validate

# skip runtime smoke (useful in CI or when API is not running)
make validate-no-smoke
```

Common overrides:

```bash
make validate SERVICE=rust-service BASE_URL=https://localhost:8800
make smoke CLEANUP_PROJECT=0
```

Runtime smoke test (hits auth/project/task/work-log/S-curve/dashboard paths):

```bash
# API must already be running
./scripts/smoke_api.sh

# optional overrides
BASE_URL=https://localhost:8800 INSECURE_TLS=1 ./scripts/smoke_api.sh
CLEANUP_PROJECT=0 ./scripts/smoke_api.sh
```

If `scurve.sqlite` is missing/outdated, refresh it first:

```bash
docker exec rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release --bin cli -- migrate-run
```

## Troubleshooting

- `failed to run migrations`: run `cargo run --bin cli -- migrate-status` and ensure `DATABASE_URL` points to the intended SQLite file.
- `cargo: not found` in container: do not wrap with `sh -lc`; run Cargo directly via `docker exec rust-service cargo ...`.
- `rustc 1.87.0 is not supported` for `time`: run with `cargo +1.88.0 ...` (or install/use Rust 1.88 toolchain in the container).
- Swagger loads but calls wrong scheme: check whether `CERT_PATH`/`KEY_PATH` are set and restart the server.
