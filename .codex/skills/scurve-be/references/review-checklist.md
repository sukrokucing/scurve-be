# Review Checklist

Use this file when reviewing a backend diff, PR, migration, or your own changes before handoff.

## 1. Purpose

Confirm the intended behavior before judging implementation details.

Ask:

- What task, ticket, endpoint behavior, or bug is this change supposed to address?
- What should be true for the user, API consumer, or operator after the change?
- Does the implementation actually solve that problem, or only part of it?
- Do tests, README examples, or OpenAPI changes agree with the intended behavior?

## 2. Edge Cases

Look for both business and technical corner cases.

Common prompts:

- Invalid or missing input
- Boundary values and empty collections
- Nullability and optional fields
- Partial failure paths and retries
- Idempotency or duplicate submission behavior
- "Impossible" states that rely on current assumptions
- Historical UUID TEXT/BLOB compatibility
- Cross-user, cross-project, or stale-membership edge cases
- Permission changes that affect hidden or indirect flows

## 3. Reliability

Treat these as high-priority review topics:

- Authentication and authorization scope
- Data integrity and transaction boundaries
- SQLite locking or migration safety
- Performance hotspots or N+1 query patterns
- Broken integrations, cache invalidation, and unsafe defaults
- Input or output validation
- Error handling that hides real failures or leaks sensitive details

## 4. Form

Check whether the solution matches repo structure and keeps acceptable cohesion and coupling.

Prefer:

- small, explicit boundaries
- simple layering between handler, service, repository, and auth logic
- low-risk abstractions
- removing duplication only when it genuinely reduces maintenance cost

## 5. Evidence

Review evidence, not only code.

Ask:

- Are tests present where the behavior changed?
- Are tests meaningful, readable, and aligned with the intended logic?
- Did required repo checks run?
- If checks were skipped, is the reason explicit and acceptable?

## 6. Clarity

Check whether intent is easy to recover quickly.

Look at:

- naming
- file placement
- function boundaries
- error messages
- comments and docs
- whether the flow can be understood without tracing every line

## 7. Taste

Keep personal preference comments clearly non-blocking unless they are backed by repo convention, measurable risk, or team agreement.

Good comment shape:

- what is wrong
- why it matters
- safer or clearer alternative at a high level

Avoid:

- vague criticism
- drive-by approval
- style-only blocking comments

## Suggested Review Output

- Verdict or readiness
- Blocking issues
- Non-blocking suggestions
- Validation or evidence gaps
- Residual risk
