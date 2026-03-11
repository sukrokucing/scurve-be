# Rust Backend Production SKILL

> Opinionated guidelines for building production-grade Rust HTTP backends (Axum / Actix / Poem).
> Extends [zhanghandong/rust-skills · coding-guidelines](https://skills.sh/zhanghandong/rust-skills/coding-guidelines).

---

## 0. Quick Reference Card

```
Naming  : snake_case fn/var · CamelCase type · SCREAMING_SNAKE_CASE const/static
Format  : rustfmt (mandatory, enforced in CI)
Lints   : #![deny(clippy::all)] · #![warn(clippy::pedantic, clippy::nursery)]
Docs    : /// public items · //! module/crate-level
Errors  : thiserror for libs · anyhow for bins/handlers
Async   : tokio runtime · no blocking calls inside .await
Logging : tracing (structured) · never println! in production
Secrets : environment variables · never in source or config files
```

---

## 1. Naming (Rust-Specific)

| Rule | Correct | Wrong |
|---|---|---|
| No `get_` prefix | `fn name()` | `fn get_name()` |
| Iterators | `iter()` / `iter_mut()` / `into_iter()` | custom ad-hoc names |
| Cheap borrow conv | `as_str()`, `as_bytes()` | — |
| Expensive conv | `to_string()`, `to_owned()` | — |
| Ownership transfer | `into_bytes()`, `into_inner()` | — |
| Static globals | `G_CONFIG` (prefix `G_`) | no prefix |
| Constants | `MAX_RETRIES` (SCREAMING) | `max_retries` |
| Meaningful lifetimes | `'ctx`, `'src`, `'req` | `'a`, `'b` |
| Boolean fields | `is_active`, `has_role` | `active`, `role` |

---

## 2. Project Structure

```
scurve-be/
├── Cargo.toml               # workspace or single crate
├── Cargo.lock               # commit this for binaries
├── .env.example             # document required env vars (no secrets)
├── .rustfmt.toml
├── .clippy.toml
├── src/
│   ├── main.rs              # bootstrap only: parse config, build router, start server
│   ├── lib.rs               # optional: expose internals for integration tests
│   ├── config.rs            # typed config from env (envy / config crate)
│   ├── error.rs             # AppError enum + IntoResponse impl
│   ├── router.rs            # route registration, middleware stacks
│   ├── db.rs                # pool creation, migration execution
│   │
│   ├── domain/              # pure business logic, NO framework deps
│   │   └── scurve/
│   │       ├── mod.rs
│   │       ├── model.rs     # data types & newtypes
│   │       ├── service.rs   # business rules
│   │       └── error.rs     # domain errors (thiserror)
│   │
│   ├── api/                 # HTTP handlers → thin adapters over domain
│   │   └── v1/
│   │       ├── mod.rs
│   │       ├── scurve.rs    # handler fns
│   │       └── dto.rs       # request/response structs + validation
│   │
│   └── infra/               # concrete implementations (DB, external APIs)
│       ├── repository/
│       └── cache/
│
└── tests/
    ├── common/mod.rs        # shared test helpers, DB fixtures
    └── api/                 # integration tests against real HTTP stack
```

**Rules:**
- `domain/` must never import from `api/` or `infra/`.
- `api/` handlers must only call `domain/` service functions — no direct DB calls.
- Keep `main.rs` under ~50 lines.

---

## 3. Data Types & Newtypes

```rust
// ✅ Newtype for domain semantics — prevents mixing primitive IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, serde::Serialize)]
#[sqlx(transparent)]
pub struct ProjectId(i64);

impl ProjectId {
    pub fn new(id: i64) -> Self { Self(id) }
    pub fn value(self) -> i64 { self.0 }
}

// ✅ Pre-allocate collections when size is known
let mut results = Vec::with_capacity(items.len());

// ✅ Slice patterns over index access
if let [first, .., last] = items.as_slice() { ... }

// ✅ Use arrays for fixed-size domain data
const PROGRESS_BUCKETS: [f64; 12] = [0.0; 12];
```

---

## 4. Error Handling

### Domain Errors (library crates)

```rust
// src/domain/scurve/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SCurveError {
    #[error("project {0} not found")]
    NotFound(ProjectId),

    #[error("invalid date range: start {start} is after end {end}")]
    InvalidDateRange { start: NaiveDate, end: NaiveDate },

    #[error("database error")]
    Db(#[from] sqlx::Error),
}
```

### Application/Handler Errors

```rust
// src/error.rs
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Validation(String),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(m)   => (StatusCode::NOT_FOUND, m),
            AppError::Validation(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
            AppError::Internal(e) => {
                tracing::error!(error = %e, "unhandled internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".into())
            }
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

// Blanket conversion so handlers can use `?` with anyhow
impl<E: Into<anyhow::Error>> From<E> for AppError { ... }
```

**Rules:**
- Never `unwrap()` in production paths. Use `expect("reason")` only for programmer invariants.
- Never expose internal error messages (DB errors, stack traces) to HTTP responses.
- Every `?` in a handler must ultimately resolve to `AppError`.
- Avoid `anyhow` in domain/library code — it erases type information callers need.

---

## 5. Async & Concurrency

```rust
// ✅ Don't hold MutexGuard across an await point
async fn process(&self) -> Result<()> {
    let value = {
        let guard = self.cache.lock().unwrap();
        guard.value.clone()           // drop guard here
    };
    do_async_work(value).await        // guard already dropped
}

// ✅ Use atomics for simple primitives
use std::sync::atomic::{AtomicU64, Ordering};
static REQUEST_COUNT: AtomicU64 = AtomicU64::new(0);
REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);

// ✅ Spawn blocking work to thread pool
let result = tokio::task::spawn_blocking(|| heavy_cpu_computation()).await?;

// ❌ Never block the async executor
std::thread::sleep(Duration::from_secs(1)); // use tokio::time::sleep
std::fs::read_to_string(path);             // use tokio::fs::read_to_string
```

**Rules:**
- Only use `async` for I/O-bound operations.
- CPU-bound work goes in `spawn_blocking`.
- Prefer `tokio::sync::Mutex` over `std::sync::Mutex` only if you must hold it across `.await`. Otherwise prefer `std::sync::Mutex` with short critical sections.
- Use `crossbeam::channel` over `std::sync::mpsc` for multi-producer multi-consumer.

---

## 6. HTTP Handler Pattern

```rust
// src/api/v1/scurve.rs

// ✅ Thin handler: validate input → call service → map to response
pub async fn get_scurve(
    State(svc): State<Arc<SCurveService>>,
    Path(project_id): Path<i64>,
    Query(params): Query<SCurveQuery>,
) -> Result<Json<SCurveResponse>, AppError> {
    params.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    let id = ProjectId::new(project_id);
    let curve = svc.calculate(id, params.into()).await?;
    Ok(Json(SCurveResponse::from(curve)))
}
```

**Rules:**
- Handlers must not contain business logic — extract to service functions.
- Always validate DTOs before passing to the domain layer.
- Use `State` extractor for dependency injection, not global statics.
- Return typed errors (`Result<T, AppError>`), not bare `StatusCode`.

---

## 7. Configuration

```rust
// src/config.rs
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub log_level: String,
    #[serde(default = "default_pool_size")]
    pub db_pool_size: u32,
}

fn default_pool_size() -> u32 { 10 }

impl Config {
    pub fn from_env() -> Result<Self, envy::Error> {
        envy::from_env::<Config>()
    }
}
```

**Rules:**
- All config comes from environment variables.
- Never hardcode secrets, URLs, or environment-specific values.
- Validate config eagerly at startup — fail fast, fail loudly.
- Commit `.env.example` with all required keys (no values).
- Never commit `.env`.

---

## 8. Database (SQLx)

```rust
// ✅ Compile-time checked queries
let row = sqlx::query_as!(
    ProjectRow,
    "SELECT id, name, start_date FROM projects WHERE id = $1",
    project_id.value()
)
.fetch_one(&pool)
.await?;

// ✅ Run migrations at startup
sqlx::migrate!("./migrations").run(&pool).await?;

// ✅ Use transactions for multi-step writes
let mut tx = pool.begin().await?;
sqlx::query!("INSERT INTO ...", ...).execute(&mut *tx).await?;
sqlx::query!("UPDATE ...", ...).execute(&mut *tx).await?;
tx.commit().await?;
```

**Rules:**
- Prefer `query_as!` / `query!` macros over raw strings for compile-time safety.
- Never construct SQL strings via format! — use bind parameters.
- Always use connection pools (`PgPool`), never single connections in handlers.
- Migrations are files in `migrations/`, named `{timestamp}_{description}.sql`.
- Repository functions return domain types, not DB row types.

---

## 9. Strings & Collections

```rust
// ✅ Borrow over clone when possible
fn process(name: &str) -> String { ... }   // not fn process(name: String)

// ✅ Use bytes for ASCII-safe operations (faster)
fn is_alphanumeric(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphanumeric())
}

// ✅ Cow<str> when you might or might not need to allocate
use std::borrow::Cow;
fn normalize(s: &str) -> Cow<str> {
    if s.contains(' ') { Cow::Owned(s.replace(' ', "_")) }
    else               { Cow::Borrowed(s) }
}

// ✅ format! over String + concatenation
let url = format!("{base}/api/v1/{path}");
```

---

## 10. Observability

```rust
// src/main.rs — structured tracing setup
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

tracing_subscriber::registry()
    .with(EnvFilter::from_default_env())   // RUST_LOG env var
    .with(tracing_subscriber::fmt::layer().json())  // JSON in prod
    .init();

// In handlers/services — use structured fields, not format strings
tracing::info!(project_id = %id, duration_ms = elapsed, "scurve calculated");
tracing::warn!(user_id = %uid, "rate limit approaching");
tracing::error!(error = %e, request_id = %req_id, "handler failed");
```

**Rules:**
- Use `tracing` everywhere — never `println!` or `eprintln!` in production code.
- Emit structured fields (key=value), not free-form strings.
- Add `request_id` / `trace_id` to every log line inside a request span.
- Use `RUST_LOG=scurve_be=debug,sqlx=warn` for granular control.
- Use `tracing-subscriber` with JSON formatter in production, pretty in dev.

---

## 11. Testing

```rust
// Unit test — domain logic, no I/O
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scurve_calculation_returns_monotonic_values() {
        let input = SCurveInput::new(100.0, 12);
        let result = calculate_scurve(input).unwrap();
        let values: Vec<f64> = result.points().map(|p| p.cumulative).collect();
        assert!(values.windows(2).all(|w| w[0] <= w[1]));
    }
}

// Integration test — full HTTP stack
// tests/api/scurve_test.rs
#[sqlx::test(fixtures("projects"))]
async fn get_scurve_returns_200(pool: PgPool) {
    let app = build_app(pool).await;
    let res = app
        .oneshot(Request::builder().uri("/api/v1/scurve/1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```

**Rules:**
- Unit-test domain logic; integration-test HTTP handlers.
- Use `sqlx::test` for DB-dependent tests — it handles per-test transactions.
- Keep test fixtures in `tests/fixtures/*.sql`.
- Test error paths explicitly — don't just test the happy path.
- CI must run `cargo test --all-features` and `cargo clippy -- -D warnings`.

---

## 12. Deprecated → Modern Alternatives

| Avoid | Use Instead | Stable Since |
|---|---|---|
| `lazy_static!` | `std::sync::LazyLock` | Rust 1.80 |
| `once_cell::Lazy` | `std::sync::LazyLock` | Rust 1.80 |
| `once_cell::sync::OnceCell` | `std::sync::OnceLock` | Rust 1.70 |
| `std::sync::mpsc` | `crossbeam::channel` | — |
| `std::sync::Mutex` (contended) | `parking_lot::Mutex` | — |
| `failure` / `error-chain` | `thiserror` + `anyhow` | — |
| `try!()` macro | `?` operator | Rust 2018 |
| `.unwrap()` in handlers | `?` + `AppError` | — |
| `println!` in lib/service | `tracing::info!` | — |
| Hardcoded config values | `envy` + env vars | — |

---

## 13. CI / Cargo Configuration

### `.cargo/config.toml`
```toml
[build]
rustflags = ["-D", "warnings"]   # treat warnings as errors

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

### Recommended `Cargo.toml` lints section
```toml
[lints.rust]
unsafe_code = "forbid"
unused_imports = "warn"

[lints.clippy]
all = "warn"
pedantic = "warn"
unwrap_used = "warn"
expect_used = "warn"     # remove if too strict; at minimum warn
```

### CI checks (in order)
1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-features`
4. `cargo audit` (from `cargo-audit` crate)
5. `cargo deny check` (license + advisory scanning)

---

## 14. Security Checklist

- [ ] No secrets in source code, config files, or git history.
- [ ] All external inputs validated and sanitized before use.
- [ ] SQL uses bind parameters exclusively (no string interpolation).
- [ ] `unsafe_code = "forbid"` in `Cargo.toml` lints.
- [ ] Dependencies audited with `cargo audit` in CI.
- [ ] Error responses never expose stack traces or internal messages.
- [ ] Timeouts set on DB pool connections and HTTP client requests.
- [ ] Rate limiting middleware on public endpoints.
- [ ] CORS configured explicitly — no wildcard `*` in production.

---

## 15. References

- [zhanghandong/rust-skills · coding-guidelines](https://skills.sh/zhanghandong/rust-skills/coding-guidelines) — base Rust rules this skill extends
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) — naming, docs, interoperability
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/) — allocation, profiling
- [tokio docs](https://docs.rs/tokio) — async runtime patterns
- [sqlx docs](https://docs.rs/sqlx) — compile-time SQL, migrations
- [axum docs](https://docs.rs/axum) — extractor pattern, state injection
- [tracing docs](https://docs.rs/tracing) — structured logging
- [thiserror](https://docs.rs/thiserror) / [anyhow](https://docs.rs/anyhow) — error handling
- [cargo-audit](https://crates.io/crates/cargo-audit) — vulnerability scanning
- [cargo-deny](https://crates.io/crates/cargo-deny) — license + advisory enforcement
