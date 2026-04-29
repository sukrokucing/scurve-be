# s-curve

Modular Axum backend for project/task S-curve management. Rust 1.88, Axum 0.7, SQLite (SQLx 0.8), JWT, RBAC.

## What You Get

- Axum 0.7 API with tracing, CORS, and rate limiting
- SQLite persistence with SQLx migrations
- JWT auth + Argon2 password hashing
- Role-Based Access Control (RBAC) with per-route permission mapping
- RBAC-filtered navigation menus with ETag caching (`GET /menus`)
- Admin view-as (`X-View-As-User`, `X-View-As-Role` headers)
- Self-service permissions endpoint (`GET /auth/me/permissions`)
- OpenAPI at `/api-docs/openapi.json` and Swagger UI at `/docs`
- Migration CLI (`make migrate`, `make migrate-status`)

## Quick Start (Docker: `rust-service`)

The `rust-service` container is usually idle (`tail -f /dev/null`), so run commands via `make` (or `docker exec` directly).

```bash
# 1) ensure db file exists and is writable
docker exec rust-service sh -c 'touch /apps/scurve-be/scurve.sqlite && chmod 664 /apps/scurve-be/scurve.sqlite'

# 2) run migrations
make migrate

# 3) start API (local-release profile — fast recompile, TLS enabled)
make run

# 4) or regenerate openapi.json first, then start
make run-openapi

# 5) production-like build (staging profile: opt-level=2, no LTO — faster than --release)
make run-release
```

Health check:

```bash
curl -sk https://localhost:8800/api/health
```

Important:

- Do not wrap with `sh -lc`; run Cargo directly via `docker exec rust-service cargo ...` (Cargo may not be in PATH via login shell).
- Current dependency graph requires Rust 1.88 (`time` crate). Use `cargo +1.88.0 ...` in the container.

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

Full variable reference:

| Variable | Default | Notes |
|---|---|---|
| `DATABASE_URL` | — | Required |
| `JWT_SECRET` | — | Required |
| `JWT_EXP_HOURS` | — | Required |
| `APP_PORT` | `8000` | `8800` in repo `.env` |
| `AUTHZ_MODE` | `strict` | `off` / `advisory` / `strict` |
| `AUTHZ_PRINCIPAL_CACHE_MS` | `30000` | In-memory principal cache TTL (ms). `0` = disabled |
| `UUID_TEXT_FAST_PATH` | `false` | Enable after UUID canonicalization migration |
| `CORS_ALLOWED_ORIGINS` | `*` | Comma-separated origins for production |
| `LOG_FORMAT` | `human` | `json` for structured log aggregators |
| `SHOW_ERRORS` | `0` | `1` = include debug detail in error responses |
| `SCURVE_COST_CURRENCY` | `USD` | Fallback ISO currency code |
| `WORKING_HOURS_PER_DAY` | `8` | Hours per calendar day used for auto progress cost calculation |
| `CERT_PATH` / `KEY_PATH` | — | Enable TLS |
| `AUTH_RATE_PER_SECOND` / `AUTH_BURST_SIZE` | `2` / `5` | Auth route rate limit |
| `GLOBAL_RATE_PER_SECOND` / `GLOBAL_BURST_SIZE` | `50` / `100` | Global rate limit |

## Migrations CLI

```bash
make migrate                # apply pending migrations (Docker)
make migrate-status         # compare applied vs pending (Docker)
```

For additional operations (create template, rollback) run the CLI directly:

```bash
docker exec -w /apps/scurve-be rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release --bin cli -- make-migration <name>

docker exec -w /apps/scurve-be rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target \
  --release --bin cli -- migrate-rollback
```

Migrations are stored in `migrations/`. Always add partial indexes `WHERE deleted_at IS NULL` for soft-delete columns.

## API Docs

- Swagger UI: `/docs`
- OpenAPI JSON: `/api-docs/openapi.json`

Auth flow in Swagger:

1. `POST /auth/register` or `POST /auth/login`
2. Click **Authorize**
3. Paste `Bearer <token>`
4. Execute protected endpoints

Regenerate `openapi.json`:

```bash
make openapi                # writes openapi.json at repo root
make run-openapi            # regenerate then start server in one step
```

## Endpoint Snapshot

### Auth

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| POST | `/auth/register` | No | — | Register user |
| POST | `/auth/login` | No | — | Login, get JWT |
| GET | `/auth/me` | Yes | — | Current user profile |
| GET | `/auth/me/permissions` | Yes | — | Current user's roles, global permissions, and per-project permissions |
| POST | `/auth/logout` | Yes | — | Stateless logout acknowledgement |
| POST | `/auth/forgot-password` | No | — | Request password reset token |
| POST | `/auth/reset-password` | No | — | Reset password with token |

### Navigation Menus

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET | `/menus` | Yes | `menu.view` | RBAC-filtered navigation menu list |

See [Navigation Menus](#navigation-menus-1) for ETag caching and frontend integration details.

### Projects

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET | `/projects` | Yes | `project.view` | List projects |
| POST | `/projects` | Yes | `project.create` | Create project |
| GET | `/projects/{id}` | Yes | `project.view` | Get project |
| PUT | `/projects/{id}` | Yes | `project.update` | Update project |
| DELETE | `/projects/{id}` | Yes | `project.delete` | Soft-delete project |
| POST | `/projects/{id}/plan` | Yes | `project.update` | Set project S-curve plan |
| DELETE | `/projects/{id}/plan` | Yes | `project.update` | Clear project S-curve plan |
| GET | `/projects/{id}/dashboard` | Yes | `project.view` | Dashboard with metric series and aggregates |
| GET | `/projects/{id}/critical-path` | Yes | `project.view` | Critical path computation |
| GET | `/projects/{id}/s-curve/health` | Yes | `project.view` | S-curve health status |
| GET | `/portfolio/s-curve/summary` | Yes | `project.view` | Portfolio-level S-curve summary |
| GET | `/users/me/projects` | Yes | — | Accessible projects with scoped permissions |

### Project Members & Resource Roles

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET | `/projects/{project_id}/members` | Yes | `project.view` | Members + access role + resource roles |
| POST | `/projects/{project_id}/members` | Yes | `project.update` | Add/update member |
| DELETE | `/projects/{project_id}/members/{user_id}` | Yes | `project.update` | Remove member |
| GET | `/resource-roles` | Yes | `project.view` | Global resource role catalog |
| POST | `/resource-roles` | Yes | `project.update` | Create resource role |
| PUT/DELETE | `/resource-roles/{id}` | Yes | `project.update` | Update/delete resource role |
| GET | `/projects/{project_id}/resource-roles` | Yes | `project.view` | Effective resource roles + project rate overrides |
| PUT | `/projects/{project_id}/resource-roles/{id}/rate` | Yes | `project.update` | Upsert project rate override |
| DELETE | `/projects/{project_id}/resource-roles/{id}/rate` | Yes | `project.update` | Remove project rate override |

### Tasks

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET | `/projects/{project_id}/tasks` | Yes | `task.view` | List tasks (filterable, sortable, paginated) |
| POST | `/projects/{project_id}/tasks` | Yes | `task.create` | Create task |
| GET | `/projects/{project_id}/tasks/{id}` | Yes | `task.view` | Get task |
| PUT | `/projects/{project_id}/tasks/{id}` | Yes | `task.update` | Update task |
| DELETE | `/projects/{project_id}/tasks/{id}` | Yes | `task.delete` | Soft-delete task |
| PUT | `/projects/{project_id}/tasks/batch` | Yes | `task.update` | Batch update tasks |
| DELETE | `/projects/{project_id}/tasks/batch` | Yes | `task.delete` | Batch soft-delete tasks |
| GET | `/projects/{project_id}/tasks/{id}/activity` | Yes | `task.view` | Task change history (automatic audit trail) |
| GET/PUT | `/projects/{project_id}/tasks/{id}/progress-components` | Yes | `task.view/update` | Weighted progress components |
| GET | `/projects/{project_id}/assignees` | Yes | `task.view` | Distinct assignees in project |
| GET/PUT | `/projects/{project_id}/task-health/rules` | Yes | `task.view/update` | Task health thresholds |

### Progress & Work Logs

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET/POST | `/projects/{project_id}/tasks/{task_id}/progress` | Yes | `progress.view/create` | Task progress entries |
| GET/PUT/DELETE | `/projects/{project_id}/tasks/{task_id}/progress/{id}` | Yes | `progress.view/create` | Single progress entry |
| GET | `/projects/{project_id}/progress` | Yes | `progress.view` | All progress in project |
| GET | `/tasks/{task_id}/progress` | Yes | `progress.view` | Legacy: progress by task id |
| GET/POST | `/projects/{project_id}/tasks/{task_id}/work-logs` | Yes | `task.view/update` | Manual labor/cost work logs |
| PUT/DELETE | `/projects/{project_id}/tasks/{task_id}/work-logs/{id}` | Yes | `task.update` | Update/delete work log |

### Notifications & Realtime

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET | `/notifications` | Yes | `project.view` | Durable notifications |
| GET | `/notifications/unread-count` | Yes | `project.view` | Unread notification count |
| POST | `/notifications/read` | Yes | `project.view` | Mark selected as read |
| POST | `/notifications/read-all` | Yes | `project.view` | Mark all as read |
| GET | `/realtime/ws` | Yes | `project.view` | WebSocket feed |

### Users & RBAC

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET | `/users` | Yes | `user.view` | List users |
| POST | `/users` | Yes | `user.manage` | Create user |
| PUT | `/users/{id}` | Yes | `user.manage` | Update user |
| DELETE | `/users/{id}` | Yes | `user.manage` | Soft-delete user |
| GET/POST/DELETE | `/rbac/roles` | Yes | `role.view/manage` | Role management |
| GET/POST/DELETE | `/rbac/permissions` | Yes | `permission.view/manage` | Permission management |
| GET/POST/DELETE | `/rbac/users/{id}/roles` | Yes | `role.manage` | Assign/revoke roles |
| GET/POST | `/rbac/users/{id}/permissions` | Yes | `permission.manage` | Direct permission grants |
| GET | `/rbac/users/{id}/effective-permissions` | Yes | `permission.view` | Resolved effective permissions |
| GET | `/rbac/audit-logs` | Yes | `role.view` | RBAC audit log |

### Admin

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| GET | `/admin/tasks/{task_id}/history` | Yes | `task.history` | Full audit trail for a task (admin/super_admin only) |

### Telemetry

| Method | Path | Auth | Permission | Purpose |
|---|---|---|---|---|
| POST | `/telemetry/events` | Yes | `telemetry.ingest` | Ingest frontend telemetry batch |

---

## Navigation Menus

`GET /menus` returns only the navigation items the current user has permission to see. The backend filters by RBAC — the frontend renders whatever it receives, no client-side permission checks needed.

### Response shape

```json
{
  "version": 1,
  "menus": [
    {
      "id": "dashboard",
      "label": "Dashboard",
      "route": "/",
      "section": "main",
      "priority": 10,
      "surfaces": ["sidebar", "bottom-nav", "search"],
      "icon": "LayoutDashboard",
      "keywords": ["home", "overview", "summary"]
    }
  ]
}
```

### Menu items and their gates

| id | label | section | required_permission |
|---|---|---|---|
| `dashboard` | Dashboard | main | _(none — always visible)_ |
| `projects` | Projects | main | `project.view` |
| `tasks` | Tasks | main | `task.view` |
| `settings` | Settings | settings | _(none — always visible)_ |
| `settings-users` | Users | settings | `user.manage` |
| `settings-roles` | Roles | settings | `role.manage` |
| `settings-policy` | Policy | settings | `role.manage` |
| `settings-flow` | Access Flow | settings | `user.manage` |

### ETag caching

The response includes `ETag` and `Cache-Control: private, max-age=300` headers. On subsequent requests send `If-None-Match: <etag>` — a `304 Not Modified` means the cached list is still valid. The ETag encodes both the menu version and the user's permission fingerprint, so it correctly invalidates when permissions change.

```http
# First request
GET /menus
Authorization: Bearer <token>
→ 200 OK
   ETag: "v1-a3f2b1c4d5e6f7a8"
   Cache-Control: private, max-age=300

# Subsequent request
GET /menus
Authorization: Bearer <token>
If-None-Match: "v1-a3f2b1c4d5e6f7a8"
→ 304 Not Modified  (no body)
```

---

## RBAC

### How it works

1. Every protected route has a `permission_name` in the `route_permissions` table.
2. On each request, the `dynamic_authz` middleware loads the caller's `Principal` (roles + permissions + project-scoped permissions) from DB, with a configurable in-memory cache (`AUTHZ_PRINCIPAL_CACHE_MS`).
3. `AUTHZ_MODE=strict` (default) denies with `403` on any permission failure. Set `AUTHZ_MODE=off` locally to bypass RBAC entirely.

### Self-service permissions (`GET /auth/me/permissions`)

Any authenticated user can call this to discover their own roles and permissions — useful for driving UI visibility:

```json
{
  "roles": ["viewer"],
  "permissions": ["menu.view", "project.view", "task.view"],
  "project_permissions": [
    {
      "project_id": "...",
      "project_name": "Alpha Project",
      "permissions": ["project.view", "task.view"]
    }
  ]
}
```

### View-as (admin only)

Admins (`admin` or `super_admin` role) can inspect the app from another user's or role's perspective by attaching a header to any request. The header only affects which permissions are checked — all writes are still attributed to the real admin's identity.

| Header | Value | Effect |
|---|---|---|
| `X-View-As-User` | target user UUID | Load that user's full principal (roles + permissions + project scopes) |
| `X-View-As-Role` | role name (e.g. `"viewer"`) | Synthetic principal with only that role's global permissions |

`X-View-As-User` takes priority when both headers are present. Non-admins sending these headers are silently ignored.

```http
GET /projects
Authorization: Bearer <admin-token>
X-View-As-User: <target-user-uuid>
```

### Adding a new permission

1. Add an entry to `permissions.json`:
   ```json
   { "name": "REPORT_VIEW", "value": "report.view" }
   ```
2. Create a migration inserting into `permissions`, assigning to roles via `role_permissions`, and adding to `route_permissions`:
   ```sql
   INSERT OR IGNORE INTO permissions (id, name, description, created_at, updated_at)
   VALUES (lower(hex(randomblob(16))), 'report.view', 'View reports', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

   INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
   SELECT r.id, p.id FROM roles r CROSS JOIN permissions p WHERE p.name = 'report.view';

   INSERT OR IGNORE INTO route_permissions (id, route_pattern, method, permission_name, created_at, updated_at)
   VALUES ('<uuid>', '/reports', 'GET', 'report.view', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
   ```
3. The constant `crate::authz::permissions::REPORT_VIEW` is available at next build (generated by `build.rs` from `permissions.json`).

---

## Task Activity Log

`GET /projects/{project_id}/tasks/{id}/activity` returns the automatic change history for a task — every create, update, and delete event with actor and changed fields.

This is powered by `activity_log` (indexed on `subject_id, event_name, occurred_at DESC`) which is written to automatically on every task mutation. No manual action required.

```json
[
  {
    "event": "task.created",
    "actor_id": "...",
    "occurred_at": "2026-04-16T09:00:00Z",
    "changes": { "title": "Re-design login page" }
  },
  {
    "event": "task.updated",
    "actor_id": "...",
    "occurred_at": "2026-04-16T10:30:00Z",
    "changes": { "status": ["todo", "in_progress"] }
  }
]
```

**Work logs** (`/work-logs` endpoints) are a separate billing concept — actual hours × resource role rate = cost. Entries are created either manually (`source: "manual"`) or automatically when task progress moves forward (`source: "auto_progress"`, calculated from `duration_days × WORKING_HOURS_PER_DAY × progress_delta`). Do not confuse these with the automatic activity log.

---

## Task List Query

`GET /projects/{project_id}/tasks` supports server-side filtering, sorting, and pagination:

| Query Param | Type | Notes |
|---|---|---|
| `q` | string | Case-insensitive title keyword search |
| `status` | string | Single or comma-separated (`todo,done`) |
| `schedule_status` | string | `finished_early`, `overdue`, `on_time`, `not_specified` |
| `health_status` | string | `ahead`, `on_track`, `at_risk`, `critical`, `needs_plan` |
| `assignee_id` | UUID | Filter by assignee |
| `start_from`, `start_to` | datetime/date | RFC3339 or `YYYY-MM-DD` |
| `due_from`, `due_to` | datetime/date | RFC3339 or `YYYY-MM-DD` |
| `sort_by` | string | `start_date`, `due_date`, `created_at`, `updated_at`, `title`, `status`, `progress`, `expected_progress_pct`, `actual_progress_pct`, `variance_pct`, `health_status` |
| `sort_dir` | string | `asc` or `desc` |
| `page` | integer | 1-based, default `1` |
| `per_page` | integer | Default `50`, max `100` |

Response includes `X-Total-Count` header with total matching rows before pagination.

---

## Task Health & Progress Model

- `progress_method`: `manual_percent_legacy` | `weighted_components`
- `execution_status`: `not_started` | `in_progress` | `blocked` | `completed`
- `health_status`: `ahead` | `on_track` | `at_risk` | `critical` | `needs_plan`
- `schedule_status`: `finished_early` | `overdue` | `on_time` | `not_specified`

Default health thresholds (project-overridable via `PUT /projects/{id}/task-health/rules`):

| Status | Variance condition |
|---|---|
| `critical` | `< -25` |
| `at_risk` | `>= -25` and `< -10` |
| `on_track` | `>= -10` and `< 10` |
| `ahead` | `>= 10` |
| `needs_plan` | No baseline set (derived, not configurable) |

For `weighted_components` tasks use `PUT /projects/{id}/tasks/{id}/progress-components` (full-set replacement). Actual progress = `SUM(weight × completion_pct) / SUM(weight)`.

If `baseline_start_at` / `baseline_end_at` are omitted, backend derives them from `start_date` / `end_date` (or `due_date` as fallback).

---

## Realtime & Notifications

WebSocket feed at `GET /realtime/ws`. Authenticate via `Authorization: Bearer <token>` header or `?token=<jwt>` query param for browser clients.

Client commands:

```json
{ "type": "subscribe",   "project_ids": ["<uuid>"], "route": "/tasks" }
{ "type": "unsubscribe", "project_ids": ["<uuid>"] }
{ "type": "ping" }
```

Event families: `notification` | `presence` | `data_changed`

WebSocket events are invalidation signals only — REST endpoints remain the source of truth for data reads.

---

## Development & Tests

Test safety model:

- Integration tests clone `scurve.sqlite` into a per-test temp file.
- Each cloned DB is cleaned before use (mutable tables truncated).
- Temp DB files are deleted automatically after each test.
- Your original `scurve.sqlite` is not modified by test runs.

`scurve.sqlite` must be migration-current before running tests:

```bash
make migrate
```

Run tests:

```bash
make test                   # all integration tests

# single test file (no make target — run directly)
docker exec rust-service cargo +1.88.0 test \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --test menus
```

One-command validation:

```bash
make validate            # smoke + fmt + tests + audit + deny
make validate-no-smoke   # skip smoke (useful in CI)
```

Runtime smoke test (API must be running):

```bash
./scripts/smoke_api.sh
BASE_URL=https://localhost:8800 INSECURE_TLS=1 ./scripts/smoke_api.sh
```

---

## Adding a New Route (checklist)

1. Add handler in `src/routes/<resource>.rs` with `#[utoipa::path(...)]`
2. Register in `src/app.rs` router
3. Add route → permission mapping via migration (`route_permissions` table)
4. Add permission to `permissions.json` (auto-generates `permissions_generated.rs` at build)
5. Add integration test
6. Run `make validate-no-smoke`

---

## Troubleshooting

- **`failed to run migrations`**: run `make migrate-status` and ensure `DATABASE_URL` points to the intended SQLite file.
- **`rustc 1.87.0 is not supported` for `time`**: run with `cargo +1.88.0 ...` (or install Rust 1.88 in the container).
- **Swagger loads but calls wrong scheme**: check whether `CERT_PATH`/`KEY_PATH` are set and restart the server.
- **`403 Route not configured`**: the route is not in `route_permissions`. Add a migration entry or set `AUTHZ_MODE=advisory` temporarily.
- **`403 Permission denied`**: the user lacks the required permission. Check `GET /auth/me/permissions` to inspect their current grants.
