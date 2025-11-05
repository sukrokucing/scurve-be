# Copilot Prompt

You are pair-programming on **s-curve**, an Axum 0.7 backend that delivers JWT-secured project and task management over SQLite using SQLx. The brief below is meant to give Copilot enough context to understand the codebase and, if necessary, recreate it from scratch.

### Project Shape
- Runtime crate: `src/main.rs` bootstraps tracing, loads env, creates the Axum app, stitches Swagger, and conditionally serves TLS via `axum-server` (Rustls).
- Core modules: `app` (Router + state), `db` (Sqlite pool init), `docs` (OpenAPI generation and Swagger routes), `errors` (`AppError` + `AppResult`), `jwt` (JWT config/extractor), `models` (domain DTOs in `user`, `project`, `task`, `progress`), `routes` (auth/projects/tasks/progress handlers), `utils` (password/Timestamp helpers).
- CLI binary in `src/bin/cli.rs` offers migration lifecycle commands backed by `sqlx::migrate`.
- SQL migrations live under `migrations/`; target output binaries land in `target/` (kept for Docker volume sharing).

### Dependencies to Expect (Cargo.toml)
`axum`, `tokio`, `sqlx` (sqlite, runtime-tokio-rustls, uuid, chrono, migrate), `serde`, `serde_json`, `jsonwebtoken`, `argon2`, `rand_core`, `uuid`, `chrono`, `anyhow`, `thiserror`, `dotenvy`, `tower-http`, `utoipa`, `utoipa-swagger-ui`, `tracing`, `tracing-subscriber`, `clap`, `async-trait`, `axum-server`.

### Database & Domain Model
- Users: `users(id TEXT PK, name, email UNIQUE, password_hash, provider='local', provider_id, created_at, updated_at, deleted_at)`.
- Projects: `projects(id TEXT PK, user_id FK -> users.id, name, description, theme_color '#3498db', created_at, updated_at, deleted_at)`.
- Tasks: `tasks(id TEXT PK, project_id FK -> projects.id, title, status 'pending', due_date, created_at, updated_at, deleted_at)`.
- Progress: `task_progress(id TEXT PK, task_id FK -> tasks.id, progress INTEGER CHECK 0..100, note TEXT, created_at, updated_at, deleted_at)`.
- All tables rely on ISO8601 timestamps (`TEXT`) and soft deletes via nullable `deleted_at`. Common indexes: `idx_projects_user_id`, `idx_tasks_project_id`.

### HTTP Surface (must stay in sync with OpenAPI)
- `/auth/register` POST → create local user, return `AuthResponse(token, user)`.
- `/auth/login` POST → verify credentials via Argon2 hash, issue JWT.
- `/auth/me` GET → require `AuthUser`, return current user.
- `/auth/logout` POST → stateless acknowledgement.
- `/projects` GET/POST → list/create authenticated projects scoped by `user_id`.
- `/projects/{id}` GET/PUT/DELETE → fetch/update/soft-delete, path params expect UUID strings.
- `/projects/{project_id}/tasks` GET/POST → list/create tasks scoped to a project (project_id path param).
- `/projects/{project_id}/tasks/{id}` PUT/DELETE → update/soft-delete specific task (project scoped).
- Progress endpoints (new):
	- `/projects/{project_id}/tasks/{task_id}/progress` GET → list progress entries for a task
	- `/projects/{project_id}/tasks/{task_id}/progress` POST → create a progress entry (body: ProgressCreateRequest)
	- `/projects/{project_id}/tasks/{task_id}/progress/{id}` PUT → update a progress entry (body: ProgressUpdateRequest)
	- `/projects/{project_id}/tasks/{task_id}/progress/{id}` DELETE → soft-delete progress entry
	- OpenAPI tag: `Progress` (progress endpoints are grouped under this tag)
- Swagger JSON at `/api-docs/openapi.json`, UI at `/docs`; docs built through `docs::build_openapi` which synthesizes missing paths, normalizes methods, injects security schemes, examples, and `servers` entries.

### Implementation Conventions
- Handlers return `AppResult<T>` and use `AppError` smart constructors (`unauthorized`, `conflict`, etc.) instead of panicking. HTTP status codes are set explicitly where behavior depends on them (e.g., `StatusCode::CREATED`).
- State extracted as `State(AppState)`; `AppState` holds `SqlitePool` and `Arc<JwtConfig>`.
- Authentication uses `AuthUser` implemented via `FromRequestParts<AppState>`, expecting `Authorization: Bearer <token>`.
- SQLx queries use `?` placeholders, fetch rows into the models defined under `crate::models`, and always filter out `deleted_at IS NOT NULL` records for soft-delete semantics.
- IDs generated with `Uuid::new_v4()`, timestamps with `utc_now()` from `utils`.
- Password hashing via `hash_password` (Argon2 + random salt) and verifying with `verify_password`.
- Request/response bodies are serde structs; keep them JSON-friendly and update `models` modules when schemas change.

### Migrations & CLI
- Run via `cargo run --bin cli -- <command>`; commands: `make-migration <name>`, `migrate-run`, `migrate-status`, `migrate-rollback`.
- `make-migration` writes `migrations/YYYY_MM_DD_HHMMSS_<sanitized>.sql` seeded with a comment placeholder.
- CLI reads env via `dotenvy` (CWD or crate `.env` fallback) and expects `DATABASE_URL`. For Docker, pass `--manifest-path scurve-be/Cargo.toml --target-dir scurve-be/target` to keep build artifacts inside the volume.

Additional migration note:
- Example new migration: `migrations/202511050001_create_task_progress.sql` creates `task_progress` and an index `idx_task_progress_task_id`.

### Environment & Deployment Notes
- Required env: `DATABASE_URL`, `JWT_SECRET`; optional `JWT_EXP_HOURS` (default 24), `APP_PORT` (default 8000); TLS enabled when `CERT_PATH` and `KEY_PATH` are provided.
- Tracing is configured via `tracing-subscriber`; use `RUST_LOG=debug` (or similar) for verbose logs.
- HTTP server binds `0.0.0.0`; TLS mode negotiated automatically via Rustls ALPN when cert/key exist.

### Style Guide
- Favor idiomatic Rust with early returns, `?`, and minimal cloning or `Arc` unless necessary.
- Keep comments terse and only for non-obvious behavior or reasoning.
- Default to ASCII unless continuing existing Unicode content.

Armed with this prompt, Copilot should be able to understand existing code and help regenerate missing pieces while respecting the architectural and stylistic contracts above.
