# Review Checklist

Use this file when reviewing a backend diff, PR, migration, or your own changes before handoff. Walk through each section in order.

## 1. Purpose

Confirm intended behavior before judging implementation.

- What task, ticket, or bug does this change address?
- What should be true for the user/API consumer after the change?
- Does the implementation solve the full problem, or only part of it?
- Do tests, README, and OpenAPI agree with the intended behavior?

## 2. Edge Cases

Look for both business and technical corner cases.

- Invalid or missing input, boundary values, empty collections
- Nullability and optional fields
- Partial failure paths and retry behavior
- Idempotency or duplicate submission
- "Impossible" states relying on current assumptions
- Historical UUID TEXT/BLOB compatibility in SQLite
- Cross-user, cross-project, stale-membership scoping
- Permission changes affecting hidden or indirect flows
- WebSocket disconnect during in-flight operations
- Rate limiter edge cases (burst vs sustained, key extraction failures)

## 3. Reliability

High-priority review topics — treat these as blocking if violated.

- **Auth & RBAC:** Correct scope checks, no permission leaks, membership validated
- **Data integrity:** Transaction boundaries correct, no partial writes on failure
- **SQLite safety:** WAL mode assumed, single-writer respected, no long-held locks, migrations tested on copy first
- **Query efficiency:** No N+1 patterns, correct use of `fetch_one` vs `fetch_optional` vs `fetch_all`, indexes present for filtered columns
- **Input/output validation:** Request bodies validated before processing, error responses don't leak internal details
- **Async correctness:** No blocking calls on tokio runtime (use `spawn_blocking` for argon2, file I/O, heavy computation), no unbounded channels or spawns without cancellation
- **Error handling:** Domain errors mapped correctly to HTTP status codes, `anyhow` not leaking into API responses, no silent error swallowing
- **Resource cleanup:** WebSocket tasks cleaned up on disconnect, temp files removed, DB connections returned to pool

## 4. Form

Check structural fit with repo patterns.

- Handler → Service → Repository layering respected
- Small, explicit boundaries between layers
- Low-risk abstractions preferred over clever ones
- Duplication removed only when it genuinely reduces maintenance cost
- New files placed in correct module directories
- Axum 0.7 patterns used consistently (not 0.8 patterns)

## 5. Evidence

Review evidence, not only code.

- Are tests present where behavior changed?
- Are tests meaningful, readable, and aligned with intended logic?
- Do tests use temp DB copies, not the production sqlite file?
- Did `make fmt`, `make test`, `make audit`, `make deny` pass?
- If `make smoke` was skipped, is the reason documented?
- For performance-sensitive changes, was any measurement done?

## 6. Clarity

Check whether intent is recoverable quickly.

- Naming: functions, types, variables convey purpose
- File placement: easy to find without grepping
- Function boundaries: small enough to understand at a glance
- Error messages: actionable for the API consumer
- Comments: explain *why*, not *what* (code explains what)
- Flow: can be understood without tracing every line

## 7. Performance

Check for patterns that degrade throughput or latency.

- **Hot path allocations:** Unnecessary `String`, `Vec`, or `clone()` in request handling
- **Blocking the runtime:** Synchronous I/O, heavy computation, or password hashing without `spawn_blocking`
- **Query waste:** Selecting unused columns, missing WHERE clauses, unbounded result sets
- **Middleware cost:** Per-request allocations in middleware, expensive clones in layers
- **Serialization:** Large response bodies that could be streamed, unnecessary intermediate serialization steps
- **Connection pool:** Pool exhaustion risk from long-held connections or missing timeouts

## 8. Taste

Keep personal preference non-blocking unless backed by convention or measurable risk.

Good review comment shape:
- What is wrong
- Why it matters
- Safer or clearer alternative at a high level

Avoid vague criticism, drive-by approvals, and style-only blocking comments.

## Review Output Format

- Verdict or readiness
- Blocking issues (with file/line references)
- Non-blocking suggestions
- Validation or evidence gaps
- Residual risk
