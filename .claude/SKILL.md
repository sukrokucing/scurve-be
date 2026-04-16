---
name: scurve-be
description: Use this skill for any Rust backend work in the scurve-be repository. Triggers include implementing or fixing endpoints, reviewing PRs or diffs, editing migrations, updating RBAC or route permissions, syncing OpenAPI specs, debugging build or Docker issues, running validation workflows, writing or fixing tests, optimizing queries or performance, working with WebSockets, or scaffolding new handlers/services/repos. Also use when the user mentions scurve, s-curve backend, or any axum/sqlx/sqlite work in this project context. Prefer this skill over general Rust knowledge whenever the task touches this repo's conventions, Docker tooling, or API contract.
---

# S-Curve Backend Skill

## Overview

Work inside the `scurve-be` repo using repo conventions, Docker-based Rust tooling, migration safety rules, RBAC guardrails, and API contract discipline. Prioritize correctness and risk before style. Keep subjective feedback non-blocking unless it violates a repo convention or creates measurable risk.

For complex tasks, decompose before implementing. Flag low-confidence areas explicitly in handoff. Keep internal reasoning private; surface only the verdict, evidence, and caveats.

## Repository Facts

| Fact | Value |
|------|-------|
| Runtime | Docker container `rust-service` |
| Toolchain | Rust `1.88.0` |
| Framework | Axum `0.7` (do NOT use 0.8 patterns) |
| Database | SQLite via SQLx `0.8`, file-based `scurve.sqlite` |
| API address | `https://localhost:8800` (TLS when certs configured) |
| Test strategy | Clone DB to temp files; never mutate original |

**Axum 0.7 pinning matters.** The repo uses axum 0.7 deliberately. Do not suggest axum 0.8 patterns such as `without_v07_checks()`, changed extractor signatures, or 0.8-style routing syntax. If upgrading is discussed, treat it as a dedicated migration task with its own PR.

## Workflow

1. Classify the request: implementation/bug fix, code review/diff review, migration/schema, RBAC/membership/permissions, API contract/OpenAPI/README sync, validation-only, or performance work.
2. For implementation tasks: decompose into endpoint logic, data model impact, RBAC implications, API contract changes, and test coverage. Make changes in the smallest safe scope. Run a self-review using the review order before handoff.
3. For review tasks: use `references/review-checklist.md` for the full prompt list. Focus on blocking issues first. Separate blocking from non-blocking findings.
4. For recurring review pain points, codify the pattern in this skill or a reference file.

## Review Order (summary)

Check in this order. Load `references/review-checklist.md` for the detailed prompt list.

1. **Purpose** — Does the change solve the intended task?
2. **Edge Cases** — Boundary values, nullability, partial failure, UUID TEXT/BLOB compat, cross-user scoping.
3. **Reliability** — Security, transactions, SQLite safety, N+1 queries, RBAC leaks, input validation.
4. **Form** — Fits repo patterns, acceptable cohesion/coupling.
5. **Evidence** — Tests present and meaningful? Validation run?
6. **Clarity** — Names, file placement, error messages readable without tracing every line.
7. **Taste** — Non-blocking unless backed by convention or measurable risk.

Mark findings as `blocking`, `important`, `non-blocking`, or `nit`. Avoid vague criticism and empty approvals.

## High-Performance Rust Patterns

These conventions apply to all new code and refactors in this repo.

### SQLite via SQLx

- **Connection pool:** Use WAL journal mode, enable foreign keys, set `synchronous = Normal`. Write pool must use `max_connections(1)` (SQLite single-writer constraint). Read pool can use higher concurrency.
- **Transactions:** Wrap multi-step writes in explicit transactions via `pool.begin()`. Prefer closure-based `conn.transaction(|txn| ...)` for automatic rollback on error.
- **Query patterns:** Use `query_as!` or `query!` macros for compile-time checked queries where practical. Avoid N+1 patterns; prefer JOINs or batch fetches. Use `.fetch_one()`, `.fetch_optional()`, `.fetch_all()` intentionally — don't default to `fetch_all` when expecting zero-or-one rows.
- **Migrations:** Use the repo's CLI migration tool only. Validate against a copied sqlite file first. If a migration changes endpoint behavior, add integration tests.

```rust
// Correct SQLite pool setup
let options = SqliteConnectOptions::from_str(&db_url)?
    .create_if_missing(true)
    .foreign_keys(true)
    .journal_mode(SqliteJournalMode::Wal)
    .synchronous(SqliteSynchronous::Normal);

let pool = SqlitePoolOptions::new()
    .max_connections(1)  // single writer for SQLite
    .connect_with(options)
    .await?;
```

### Axum Handlers & Middleware

- **State:** Use `State<AppState>` extractor with `Router::with_state()`. For mutable shared state, use `Arc<RwLock<T>>` or `Arc<Mutex<T>>` depending on read/write ratio.
- **Middleware stacking:** Use `ServiceBuilder` for composing layers. Order matters — error handling layers go above the layers that produce errors (e.g., `HandleErrorLayer` above `TimeoutLayer`).
- **Error handling:** Return `impl IntoResponse` from handlers. Map domain errors to HTTP status codes via the repo's error types. Use `thiserror` for domain errors, `anyhow` for infrastructure/startup errors.
- **Extractors:** Extract in handler signature order: `State`, `Path`, `Query`, `Headers`, then `Json` body last. Reject early with typed errors, not panics.

```rust
// Correct middleware ordering
let app = Router::new()
    .route("/api/resource", get(list).post(create))
    .layer(
        ServiceBuilder::new()
            .layer(HandleErrorLayer::new(|err: BoxError| async move {
                StatusCode::REQUEST_TIMEOUT
            }))
            .layer(TimeoutLayer::new(Duration::from_secs(30)))
            .layer(TraceLayer::new_for_http())
            .layer(cors_layer)
    )
    .with_state(state);
```

### WebSocket Conventions

The repo uses axum's `ws` feature with `futures-util` for stream splitting.

- Accept via `WebSocketUpgrade` extractor, upgrade with `.on_upgrade()`.
- Split socket with `.split()` into sender/receiver, run each in a spawned task.
- Use `tokio::select!` to clean up both tasks when either side disconnects.
- For broadcast patterns, use `tokio::sync::broadcast` channel shared via app state.

### Error Response Convention

All API errors return a consistent JSON shape. Map domain errors through a central error type.

```rust
// Standard error response shape
{
    "error": {
        "code": "RESOURCE_NOT_FOUND",
        "message": "Project not found"
    }
}

// Domain errors use thiserror
#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "RESOURCE_NOT_FOUND"),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };
        // ... build JSON body
    }
}
```

### Performance Checklist

Apply when writing or reviewing handlers, queries, or middleware:

- Avoid allocations in hot paths: prefer `&str` over `String`, use `Cow<'_, str>` for conditional ownership.
- Use `#[inline]` sparingly — only on small functions called across crate boundaries.
- Prefer `tower::ServiceBuilder` to compose layers once, not per-request.
- For large response bodies, consider streaming with `axum::body::Body::from_stream()`.
- Profile before optimizing: use `cargo flamegraph` or `tokio-console` for async bottlenecks.
- Keep handler functions small — extract business logic into service layer for testability and to keep the async executor unblocked.
- Use `tokio::task::spawn_blocking` for CPU-heavy synchronous work (e.g., argon2 password hashing).

### Dependency Awareness

| Crate | Version | Notes |
|-------|---------|-------|
| axum | 0.7 | Pinned. Do not use 0.8 APIs. |
| sqlx | 0.8 | Use `runtime-tokio-rustls`, `migrate`, `macros` features. |
| tower-http | 0.5 | CORS + trace features. |
| tower_governor | 0.4.2 | Rate limiting. Check compat before upgrading. |
| argon2 | 0.5 | Password hashing — always `spawn_blocking`. |
| jsonwebtoken | 9 | JWT auth. |
| utoipa | 4 | OpenAPI spec generation. |

Version policy: pin major versions, update patch/minor when tests pass. Major upgrades get their own PR with migration notes.

## RBAC + Membership + Work Log Guardrails

- System RBAC roles and project resource roles are separate concepts.
- Cross-user operations must enforce scoped permission checks.
- Work logs validate project membership and allowed resource-role assignment.
- Prefer explicit 403 or 404 without leaking existence details.
- Treat RBAC checks as reliability issues, not optional polish.

## API Contract Discipline

When endpoint, request, response, or auth behavior changes:

1. Update handler → service → repository code.
2. Add or update integration tests in `tests/`.
3. Regenerate OpenAPI: `cargo run --bin dump_openapi -- --port 8000 --out openapi.json` and `--port 8800 --out openapi-live.json`.
4. Update `README.md` if externally visible.
5. Sync route permissions with protected endpoints.

## Docker Commands

All commands run via `docker exec rust-service` with `--manifest-path /apps/scurve-be/Cargo.toml --target-dir /apps/scurve-be/target`.

```bash
# Compile + run (iterative development)
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  cargo +1.88.0 run --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --profile local-release

# Compile once, then run binary (fast restarts)
docker exec rust-service cargo +1.88.0 build \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --profile local-release

docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  /apps/scurve-be/target/local-release/s-curve

# Create migration
docker exec rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --bin cli -- make-migration <n>

# Apply migration
docker exec rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --release --bin cli -- migrate-run
```

## Before Handoff

Run and pass (or document skip reason):

1. `make fmt` → `make test` → `make audit` → `make deny`
2. `make smoke` (when API is running)
3. Self-review using the review order above
4. Migration safety validated (if schema touched)
5. OpenAPI and README synced (if API changed)
6. RBAC ownership and scoping validated (if auth/resource logic changed)
7. No destructive DB or test side-effects left behind

Handoff format:

1. **Result** — What changed, decided, or recommended.
2. **Confidence** — `0.0–1.0`.
3. **Validation** — Tests/checks run.
4. **Findings** — Blocking first, then non-blocking.
5. **Caveats** — Remaining risks, skipped checks, assumptions.

## Performance Guardrails

- Prefer `--profile local-release` during development.
- Reuse `/apps/scurve-be/target` for warm incremental cache.
- Avoid unnecessary clean builds.
- Use `rg` for code search; `cargo test --test <n>` for focused test loops.
- Keep smoke checks deterministic; clean up test-created data.

## Context7 MCP Usage

Use Context7 for external framework/library docs only. Prefer local source for repo behavior.

```bash
# Health check
docker exec -i fe npx -y @upstash/context7-mcp --help
docker exec -i fe npx -y @playwright/mcp@latest --help
```
