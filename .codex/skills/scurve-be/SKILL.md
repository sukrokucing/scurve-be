---
name: scurve-be
description: Use this skill for backend work in the scurve-be repository (Rust + Axum + SQLite + RBAC + OpenAPI), especially when implementing or fixing endpoints, editing migrations, updating authorization or route-permission behavior, syncing OpenAPI or README examples, or running the repo's required validation and safety workflow. It provides the fast Docker loop, migration and DB safeguards, API-contract sync steps, and concise risk-aware handoff guidance.
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

## Reasoning and Response Policy

For complex backend tasks, use a lightweight structured reasoning loop before making changes.

1. **Decompose** the task into sub-problems such as endpoint logic, data model or migration impact, RBAC implications, API contract changes, and test coverage.
2. **Solve** each sub-problem separately and track confidence privately on a `0.0-1.0` scale.
3. **Verify** for logic errors, factual mismatch with repo conventions, incomplete scope, security regressions, data integrity risk, and biased assumptions.
4. **Synthesize** the final answer or implementation plan by giving extra weight to the lowest-confidence or highest-risk areas.
5. **Reflect** before handoff. If overall confidence is below `0.8`, identify the weakest assumption, re-check the relevant code, tests, or docs, and revise once before responding.

For simple or localized tasks, skip the full loop and answer directly.

Do not expose full internal chain-of-thought. When useful, provide a concise external summary with:
- clear answer or implementation result
- confidence level
- key caveats, validation gaps, or remaining risks

For implementation-heavy requests, prioritize:
- what changed
- files touched
- tests and validation run
- remaining caveats or follow-up risk

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

Whenever endpoint, request, response, or auth behavior changes:

1. Update handler, service, and repository code.
2. Update or add integration tests in `tests/`.
3. Regenerate OpenAPI snapshots:

```bash
cargo run --bin dump_openapi -- --port 8000 --out openapi.json
cargo run --bin dump_openapi -- --port 8800 --out openapi-live.json
```

4. Update `README.md` endpoint or docs examples if externally visible.
5. Ensure route permissions are synchronized with protected endpoints.

## RBAC + Membership + Work Log Guardrails

- Treat system RBAC roles and project resource roles as separate concepts.
- Cross-user operations must enforce scoped permission checks.
- Work logs must validate project membership and allowed resource-role assignment.
- Prefer explicit 403 or 404 behavior without leaking sensitive existence details.

## Performance Guardrails for This Repo

- Prefer `--profile local-release` during iterative development.
- Reuse `/apps/scurve-be/target` to keep incremental cache warm.
- Avoid unnecessary clean builds.
- Use `rg` for code search and `cargo test --test <name>` for focused loops.
- Keep smoke checks deterministic and clean up test-created project data.

## Context7 MCP Usage (Docs Lookup)

Use Context7 for external docs only for framework or library behavior. Prefer local source first for repo behavior.

Quick health probe for Context7 client environment:

```bash
docker exec -i fe npx -y @upstash/context7-mcp --help
```

Quick health probe for Playwright MCP client:

```bash
docker exec -i fe npx -y @playwright/mcp@latest --help
```

If MCP handshake fails, verify `fe` container is running and re-check `~/.codex/config.toml` MCP command paths.

## Handoff Format

When handing work back to the user, use this compact structure when relevant:

1. **Answer / Result** - What was changed, decided, or recommended.
2. **Confidence** - Overall confidence from `0.0-1.0`.
3. **Validation** - Tests, checks, or commands run.
4. **Key Caveats** - Remaining risks, skipped checks, or assumptions.

Keep the handoff concise. Prefer specifics over narration.

## Completion Checklist

- [ ] Build, test, audit, deny, and smoke done (or documented skip reason)
- [ ] Migration safety validated (if schema touched)
- [ ] OpenAPI and README synced (if API changed)
- [ ] RBAC ownership and scoping validated (if auth or resource logic changed)
- [ ] No destructive DB or test side-effects left behind
