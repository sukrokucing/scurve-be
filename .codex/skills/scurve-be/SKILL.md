---
name: scurve-be
description: use this skill for backend work in the scurve-be repository (rust + axum + sqlite + rbac + openapi), especially when implementing or fixing endpoints, reviewing backend diffs or pull requests, editing migrations, updating authorization or route-permission behavior, syncing openapi or readme examples, or running the repo's validation and safety workflow. it provides the fast docker loop, migration and db safeguards, api-contract sync steps, and a structured review and self-review workflow that prioritizes purpose, edge cases, reliability, evidence, and concise risk-aware handoff.
---

# S-Curve Backend

## Overview

Work inside the `scurve-be` repo using repo conventions, Docker-based Rust tooling, migration safety rules, RBAC guardrails, and API contract discipline.

When the task is code review, diff review, or self-review before handoff, prioritize correctness and risk before style. Keep subjective feedback clearly non-blocking unless it violates an explicit repo convention or creates measurable risk.

## Repository Facts

- Primary runtime is Docker container `rust-service`.
- Toolchain requirement is Rust `1.88.0` (not `1.87.0`).
- API commonly runs on TLS `https://localhost:8800` when certs are configured.
- SQLite DB is file-based (`scurve.sqlite`) and must be handled carefully.
- Existing test suite is designed to avoid mutating the original DB by cloning to temp DBs.

## Workflow

1. Classify the request as one of:
   - implementation or bug fix
   - code review, diff review, or self-review
   - migration or schema work
   - RBAC, membership, or permission work
   - API contract, OpenAPI, or README sync
   - validation-only request
2. For implementation-heavy tasks:
   - decompose into endpoint logic, data model or migration impact, RBAC implications, API contract changes, and test coverage
   - make changes in the smallest safe scope that solves the task
   - run a self-review using the review order below before handoff
3. For review-heavy tasks:
   - use the review order below and focus first on blocking issues
   - separate blocking findings from non-blocking suggestions
   - avoid vague or taste-only comments
4. For recurring review pain points, turn the pattern into an explicit convention in this skill or a reference file.

## Review Order

When reviewing code, PRs, diffs, tests, or your own changes before handoff, check in this order. For a deeper prompt list, use `references/review-checklist.md`.

1. **Purpose** - Confirm the change solves the intended task, ticket, or user-visible behavior. If intent is unclear, infer it from ticket text, tests, README examples, OpenAPI, or surrounding code before judging implementation details.
2. **Edge Cases** - Look for business and technical corner cases such as omitted requirements, boundary values, invalid inputs, nullability, partial failure behavior, impossible states, idempotency, race-like sequences, historical UUID TEXT/BLOB compatibility, and cross-user or cross-project scoping edge cases.
3. **Reliability** - Check for security, performance, data integrity, transactional safety, broken integrations, auth leaks, cache invalidation, query inefficiency, input or output validation, and SQLite safety. Treat RBAC and membership checks as reliability issues, not optional polish.
4. **Form** - Check whether the solution fits repo patterns and keeps acceptable cohesion and coupling. Prefer simple layering, explicit boundaries, and low-risk abstractions over cleverness.
5. **Evidence** - Ensure tests and required validation support the change. Review tests with the same rigor as production code. If validation is skipped, say so explicitly and explain why.
6. **Clarity** - Check whether names, file placement, error handling, API shape, and comments make intent easy to understand without reading every line. Prefer code that can be read diagonally.
7. **Taste** - Treat personal preferences as non-blocking unless they are backed by repo conventions, measurable maintenance risk, or a clear team agreement.

## Review Comment Rules

- State what is wrong, why it matters, and a high-level fix or safer direction.
- Avoid vague comments like `this is bad` or empty approvals like `LGTM`.
- Mark findings by impact when useful: `blocking`, `important`, `non-blocking`, `nit`.
- Do not block on taste-only feedback.
- Prefer concrete references to files, queries, tests, or repo conventions.

## Reasoning and Response Policy

For complex backend tasks, use a lightweight structured reasoning loop before making changes.

1. **Decompose** the task into sub-problems such as endpoint logic, data model or migration impact, RBAC implications, API contract changes, and test coverage.
2. **Solve** each sub-problem separately and track confidence privately on a `0.0-1.0` scale.
3. **Verify** for logic errors, factual mismatch with repo conventions, incomplete scope, security regressions, data integrity risk, and biased assumptions.
4. **Synthesize** the final answer or implementation plan by giving extra weight to the lowest-confidence or highest-risk areas.
5. **Reflect** before handoff. If overall confidence is below `0.8`, identify the weakest assumption, re-check the relevant code, tests, or docs, and revise once before responding.

Do not expose full internal chain-of-thought. When useful, provide a concise external summary with:

- clear answer or implementation result
- confidence level
- key caveats, validation gaps, or remaining risks

For implementation-heavy requests, prioritize:

- what changed
- files touched
- tests and validation run
- remaining caveats or follow-up risk

For review-heavy requests, prioritize:

- verdict or readiness
- blocking issues first
- non-blocking suggestions second
- evidence or validation gaps
- residual risk

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

Before handoff or requesting peer review, run a self-review using the review order above.

Run and pass when applicable:

1. `make fmt`
2. `make test`
3. `make audit`
4. `make deny`
5. `make smoke` (when API is running)

If a step is skipped, explicitly state why.

Prefer automated checks over manual style policing when a formatter, test, lint, or scripted validation can enforce the rule.

## Migration and DB Rules

- Use migrations only; do not edit schema ad-hoc in production DB.
- Validate migrations against a dummy copied sqlite first.
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

## Context7 MCP Usage

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
4. **Review Findings** - For review tasks, list blocking issues first, then non-blocking suggestions.
5. **Key Caveats** - Remaining risks, skipped checks, or assumptions.

Keep the handoff concise. Prefer specifics over narration. Do not use empty approvals without reasons.

## Completion Checklist

- [ ] Build, test, audit, deny, and smoke done (or documented skip reason)
- [ ] Self-review completed using the review order above
- [ ] Migration safety validated (if schema touched)
- [ ] OpenAPI and README synced (if API changed)
- [ ] RBAC ownership and scoping validated (if auth or resource logic changed)
- [ ] No destructive DB or test side-effects left behind
