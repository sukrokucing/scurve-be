---
name: scurve-be
description: Use this skill for any backend work in the scurve-be repository (Rust + Axum + SQLite + RBAC + OpenAPI). It provides the fast Docker workflow, safe migration/testing rules, API-contract sync steps, and performance guardrails.
---

# S-Curve Backend Skill

## Use This Skill When

- Implementing or fixing backend endpoints in `src/`
- Editing SQL migrations in `migrations/`
- Updating RBAC route-permission behavior
- Changing API schemas/examples in OpenAPI or README docs
- Running the quality/safety gate before handoff

## Repository Facts (Do Not Assume Otherwise)

- Primary runtime is Docker container `rust-service`.
- Toolchain requirement is Rust `1.88.0` (not `1.87.0`).
- API commonly runs on TLS `https://localhost:8800` when certs are configured.
- SQLite DB is file-based (`scurve.sqlite`) and must be handled carefully.
- Existing test suite is designed to avoid mutating the original DB by cloning to temp DBs.

## Fast Development Loop

### 1) Start API quickly (Docker)

```bash
# compile + run (faster day-to-day profile)
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  cargo +1.88.0 run --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --profile local-release
```

### 2) Compile once, then run binary (fast restarts)

```bash
# compile once
docker exec rust-service cargo +1.88.0 build \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --profile local-release

# run binary directly
docker exec rust-service env RUST_BACKTRACE=1 \
  CERT_PATH=/apps/certs/cert.pem KEY_PATH=/apps/certs/key.pem \
  /apps/scurve-be/target/local-release/s-curve
```

### 3) Safety gate

```bash
# in repo root (host)
make validate

# if API is not currently running
make validate-no-smoke
```

## Required Quality Gate Before Handoff

Run and pass:

1. `make fmt`
2. `make test`
3. `make audit`
4. `make deny`
5. `make smoke` (when API is running)

If a step is skipped, explicitly state why.

## Migration and DB Rules

- Use migrations only; do not edit schema ad-hoc in production DB.
- Validate migrations against a **dummy copied sqlite** first.
- Keep compatibility for historical UUID storage differences (TEXT/BLOB) where relevant.
- If migration changes endpoint behavior, add or update integration tests.

Recommended migration flow:

```bash
# create migration
docker exec rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --bin cli -- make-migration <name>

# apply migration
docker exec rust-service cargo +1.88.0 run \
  --manifest-path /apps/scurve-be/Cargo.toml \
  --target-dir /apps/scurve-be/target --release --bin cli -- migrate-run
```

## API Contract Discipline

Whenever endpoint/request/response behavior changes:

1. Update handler/service/repository code.
2. Update or add integration tests in `tests/`.
3. Regenerate OpenAPI snapshots:

```bash
cargo run --bin dump_openapi -- --port 8000 --out openapi.json
cargo run --bin dump_openapi -- --port 8800 --out openapi-live.json
```

4. Update `README.md` endpoint/docs examples if externally visible.
5. Ensure route permissions are synchronized with protected endpoints.

## RBAC + Membership + Work Log Guardrails

- Treat system RBAC roles and project resource roles as separate concepts.
- Cross-user operations must enforce scoped permission checks.
- Work logs must validate project membership and allowed resource-role assignment.
- Prefer explicit 403/404 behavior without leaking sensitive existence details.

## Performance Guardrails for This Repo

- Prefer `--profile local-release` during iterative development.
- Reuse `/apps/scurve-be/target` to keep incremental cache warm.
- Avoid unnecessary clean builds.
- Use `rg` for code search and `cargo test --test <name>` for focused loops.
- Keep smoke checks deterministic and cleanup test-created project data.

## Context7 MCP Usage (Docs Lookup)

Use Context7 for external docs only (framework/libraries). Prefer local source first for repo behavior.

Quick health probe for Context7 client environment:

```bash
docker exec -i fe npx -y @upstash/context7-mcp --help
```

Quick health probe for Playwright MCP client:

```bash
docker exec -i fe npx -y @playwright/mcp@latest --help
```

If MCP handshake fails, verify `fe` container is running and re-check `~/.codex/config.toml` MCP command paths.

## Completion Checklist

- [ ] Build/test/audit/deny/smoke done (or documented skip reason)
- [ ] Migration safety validated (if schema touched)
- [ ] OpenAPI + README synced (if API changed)
- [ ] RBAC ownership/scoping validated (if auth/resource logic changed)
- [ ] No destructive DB/test side-effects left behind
