# scurve-be — Claude Code Guide

Modular Axum backend for project/task S-curve management. Rust 1.88, Axum 0.7, SQLite (SQLx 0.8), JWT, RBAC.

## Source Layout

```
src/
  main.rs            — entry point: tracing, TLS, server bind
  app.rs             — AppState, router assembly, CORS, rate limiting, body limit
  errors.rs          — AppError enum + IntoResponse (AppResult<T> alias)
  utils.rs           — hash_password, verify_password, utc_now, round2, parse_db_datetime
  task_metrics.rs    — health/schedule/progress derived field computation
  events/mod.rs      — DomainEvent, EventBus, start_activity_listener
  realtime.rs        — RealtimeHub, WebSocket handler, start_realtime_dispatcher
  authz/
    mod.rs           — AuthzMode enum (Off/Advisory/Strict), well-known role/perm names
    principal.rs     — Principal::load (in-memory cache), invalidate_cache_for
    layer.rs         — dynamic_authz middleware
    route_cache.rs   — RoutePermissionCache loaded from DB at startup
  db/
    mod.rs           — SQLitePoolOptions, PRAGMAs, sqlx::migrate!()
    row_parsers.rs   — parse_datetime_value (canonical), db_user_from_row, etc.
    uuid_sql.rs      — match_uuid_clause, case_uuid, expr_uuid (dual blob/text UUID)
  routes/            — one file per resource (auth, projects, tasks, progress, ...)
  models/            — serde structs + DB row types per resource
```

## Key Conventions

### Error handling
Always return `AppResult<T>` (= `Result<T, AppError>`). Never use ad-hoc `(StatusCode, Json<...>)` error types — use `AppError::bad_request(...)`, `AppError::not_found(...)`, etc. `AppError` logs itself via `tracing::error!` in `IntoResponse`.

### Shared helpers — use these, don't redefine
- `crate::utils::round2(f64) -> f64` — 2dp rounding
- `crate::utils::parse_db_datetime(&str) -> AppResult<DateTime<Utc>>` — delegates to `db::row_parsers::parse_datetime_value`
- `crate::utils::utc_now() -> DateTime<Utc>`

### UUID handling (dual blob/text storage)
Use `db::uuid_sql::match_uuid_clause(col)` for WHERE predicates — always requires **two** `.bind()` calls per usage (text + blob). Use `db::uuid_sql::case_uuid(col)` in SELECT to normalize output. Set `UUID_TEXT_FAST_PATH=1` in `.env` after verifying all UUIDs are canonicalized (migration `20260312093000`).

### Authz
- `AUTHZ_MODE` defaults to `Strict`. Set `AUTHZ_MODE=off` locally to bypass RBAC.
- `AUTHZ_PRINCIPAL_CACHE_MS` defaults to `30000` (30 s). Set to `0` to disable.
- `Principal::invalidate_cache_for(user_id)` — call after any role/permission mutation.

### Database writes
- Always wrap multi-statement mutations in `pool.begin() / tx.commit()`.
- Use `.execute(&mut *tx)` inside transactions.
- SQLite pool: `max_connections=5` — WAL serializes writers, don't raise this.

### Event bus
`EventBus` is `broadcast::Sender<Value>`. Receivers loop with explicit `Lagged` handling:
```rust
loop {
    match rx.recv().await {
        Ok(event) => { /* handle */ }
        Err(broadcast::error::RecvError::Lagged(n)) => { warn!(...); continue; }
        Err(broadcast::error::RecvError::Closed) => break,
    }
}
```
Never use `while let Ok(event) = rx.recv().await` — silently exits on lag.

### Activity logging
`events::log_activity_with_context(&state.event_bus, "created", Some(actor_id), &entity, old_entity, Some(ctx))` — fire-and-forget. Entity must implement `Loggable`.

## Environment Variables

| Variable | Default | Notes |
|---|---|---|
| `DATABASE_URL` | — | Required. `sqlite:///apps/scurve-be/scurve.sqlite` in Docker |
| `JWT_SECRET` | — | Required |
| `JWT_EXP_HOURS` | — | Required |
| `APP_PORT` | `8000` | `8800` in repo `.env` |
| `AUTHZ_MODE` | `strict` | `off` / `advisory` / `strict` |
| `AUTHZ_PRINCIPAL_CACHE_MS` | `30000` | `0` = no cache |
| `UUID_TEXT_FAST_PATH` | `false` | Enable after UUID canonicalization |
| `CORS_ALLOWED_ORIGINS` | `*` | Comma-separated origins for production |
| `LOG_FORMAT` | human | `json` for structured log aggregators |
| `SHOW_ERRORS` | `0` | `1` = include debug detail in error responses |
| `SCURVE_COST_CURRENCY` | `USD` | Fallback ISO currency |
| `CERT_PATH` / `KEY_PATH` | — | Enable TLS |
| `AUTH_RATE_PER_SECOND` / `AUTH_BURST_SIZE` | `2` / `5` | Auth route rate limit |
| `GLOBAL_RATE_PER_SECOND` / `GLOBAL_BURST_SIZE` | `50` / `100` | Global rate limit |

## Running (Docker)

```bash
# check / compile
docker exec rust-service cargo +1.88.0 check \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target

# run tests
docker exec rust-service cargo +1.88.0 test \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --tests

# start API (local-release profile = fast recompile)
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  cargo +1.88.0 run --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --profile local-release

# full validation (smoke + fmt + tests + audit + deny)
make validate
make validate-no-smoke   # skip smoke, useful when API is not running
```

Must use `cargo +1.88.0` in the container — the `time` crate requires Rust ≥ 1.88. Do NOT wrap with `sh -lc '...'` in Docker (Cargo not in PATH that way).

## Tests

Tests live in `tests/`. 35+ integration test files. Each uses `cloned_clean_db()` from `tests/support/db.rs` which:
1. Copies `scurve.sqlite` into a `tempfile` directory
2. Truncates all mutable tables
3. Drops the temp file after the test

**Important:** `scurve.sqlite` must exist and be migration-current before running tests. Refresh with:
```bash
docker exec rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release --bin cli -- migrate-run
```

Contract tests (`openapi_contract_sync.rs`, `route_permissions_sync.rs`) catch schema drift between code and DB — run these when adding routes or permissions.

## Adding a New Route

1. Add handler in `src/routes/<resource>.rs` with `#[utoipa::path(...)]` annotation
2. Register in `src/app.rs` router
3. Add route→permission mapping in the DB via a migration
4. Add the permission name to `permissions.json` (regenerates `permissions_generated.rs` at build)
5. Add integration test
6. Run `make validate-no-smoke`

## Migrations

```bash
cargo run --bin cli -- make-migration <name>   # create template
cargo run --bin cli -- migrate-run             # apply pending
cargo run --bin cli -- migrate-status          # compare applied vs pending
cargo run --bin cli -- migrate-rollback        # rollback last
```

Migrations are in `migrations/`. Always add partial indexes `WHERE deleted_at IS NULL` for soft-delete columns.

## Soft Deletes

All mutable entities use `deleted_at IS NULL` filter. Always include `AND deleted_at IS NULL` in SELECT/UPDATE WHERE clauses. Never hard-delete.

## Request Body Limit

1 MiB enforced at the router level (`DefaultBodyLimit::max(1_048_576)`). Returns `413` if exceeded.

## Known Deferred Items (not yet implemented)

- `POST /auth/forgot-password` generates a reset token but does NOT send email — add email integration and remove the `TODO` in `src/routes/auth.rs`
- JWT logout is stateless (no revocation) — consider a short-lived blocklist or refresh tokens
- No `/metrics` endpoint — consider `axum-prometheus` for Prometheus scraping
- No `X-Request-ID` correlation header — add `MakeSpan` to `TraceLayer`
- `list_tasks` still fetches all rows before applying `health_status`/`schedule_status` filters (these are computed, not stored) — future: materialise as stored columns
- `projects.rs` is ~2900 lines — future: split into `dashboard.rs`, `s_curve.rs`, `critical_path.rs` sub-modules

## graphify

This project has a graphify knowledge graph at graphify-out/.

Rules:
- Before answering architecture or codebase questions, read graphify-out/GRAPH_REPORT.md for god nodes and community structure
- If graphify-out/wiki/index.md exists, navigate it instead of reading raw files
- For cross-module "how does X relate to Y" questions, prefer `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` over grep — these traverse the graph's EXTRACTED + INFERRED edges instead of scanning files
- After modifying code files in this session, run `graphify update .` to keep the graph current (AST-only, no API cost)
