# Backend Implementation Plan: GET /users (User Listing API)

Purpose
- Provide a minimal, secure users listing endpoint for the Access Flow Explorer UI (`/admin/flow`).
- Allow the frontend to search users by name/email and fetch basic user info (id, name, email) to drive the Users column.
- Keep payload minimal and paginated to remain safe for large userbases.

High-level requirements
- Path: `GET /users`
- Auth: Bearer token required. Endpoint should be accessible to admin users or roles with `users:read` privilege.
- Query params:
  - `q` (optional): string; search by name or email (case-insensitive, partial match)
  - `page` (optional): int; 1-based, default `1`
  - `per_page` (optional): int; default `25`, max `100`
- Response: 200 OK with array of `User` objects or empty array if none. Support `X-Total-Count` header for client pagination.
- Stable contract: frontend will fallback to mock data if 404 returned; prefer returning 200+empty array if endpoint present.

OpenAPI snippet (reference)
```json
"/users": {
  "get": {
    "operationId": "list_users",
    "parameters": [
      { "name": "q", "in": "query", "required": false, "schema": { "type": "string" }, "description": "Optional search query (name or email)" },
      { "name": "page", "in": "query", "required": false, "schema": { "type": "integer", "default": 1 } },
      { "name": "per_page", "in": "query", "required": false, "schema": { "type": "integer", "default": 25 } }
    ],
    "responses": {
      "200": {
        "description": "List users",
        "content": {
          "application/json": {
            "schema": { "type": "array", "items": { "$ref": "#/components/schemas/User" } }
          }
        }
      }
    },
    "security": [ { "bearerAuth": [] } ],
    "summary": "List users (admin)",
    "tags": [ "Users" ]
  }
}
```

`User` schema (re-use existing schema in OpenAPI)
- Fields to return (minimal):
  - `id` (uuid)
  - `name` (string)
  - `email` (string)
  - `provider` (string)
  - `provider_id` (string | null) optional
  - `created_at`, `updated_at` (datetime)

Response example
```json
[{
  "id": "00000000-0000-0000-0000-000000000001",
  "name": "Admin User",
  "email": "admin@example.com",
  "provider": "local",
  "provider_id": null,
  "created_at": "2025-12-01T10:00:00Z",
  "updated_at": "2025-12-01T10:00:00Z"
}]
```

Pagination headers
- Include `X-Total-Count` (integer) to help the frontend compute total pages.
- Alternatively return a meta object: `{ items: [...], total: 123 }` — either is acceptable; prefer headers for minimal payload changes.

Search semantics
- `q` should perform a case-insensitive substring match against `name` and `email`.
- For better UX, support wildcard whitespace-insensitive search and basic tokenization.
- Use DB-side `ILIKE '%q%'` (Postgres) or full-text search if available for performance on large datasets.

Access control and auditing
- Only allow requests from authenticated admins or roles with explicit permission (`users:read`).
- Log access attempts (successful/failed) for audit trails. Redact sensitive fields in logs.

Rate limiting and abuse
- Apply a sensible per-IP or per-client rate limit (e.g. 60 requests/min) to protect the endpoint.

Implementation examples

1) Example (Express.js) — simplified
```js
// GET /users?q=&page=&per_page=
app.get('/users', ensureAdminOrHasScope('users:read'), async (req, res) => {
  const q = req.query.q?.toString();
  const page = Math.max(1, parseInt(req.query.page as string || '1'));
  const per_page = Math.min(100, parseInt(req.query.per_page as string || '25'));

  // Basic SQL: use parameterized queries
  const offset = (page - 1) * per_page;
  if (q) {
    // Use ILIKE for Postgres
    const rows = await db.query('SELECT id, name, email, provider, provider_id, created_at, updated_at FROM users WHERE name ILIKE $1 OR email ILIKE $1 ORDER BY name LIMIT $2 OFFSET $3', [`%${q}%`, per_page, offset]);
    const total = (await db.query('SELECT count(*) FROM users WHERE name ILIKE $1 OR email ILIKE $1', [`%${q}%`])).rows[0].count;
    res.set('X-Total-Count', total);
    return res.json(rows.rows);
  } else {
    const rows = await db.query('SELECT id, name, email, provider, provider_id, created_at, updated_at FROM users ORDER BY name LIMIT $1 OFFSET $2', [per_page, offset]);
    const total = (await db.query('SELECT count(*) FROM users')).rows[0].count;
    res.set('X-Total-Count', total);
    return res.json(rows.rows);
  }
});
```

2) Example (Rust / Actix) — pseudo
- Use prepared statements and parameterized queries; return JSON array; set header `X-Total-Count`.

DB and indices
- Ensure an index exists on `email` and (optionally) `lower(name)` for case-insensitive searches. Example (Postgres):
  - `CREATE INDEX idx_users_email ON users (email);`
  - `CREATE INDEX idx_users_lower_name ON users (lower(name));`
- If using full-text search, index `to_tsvector(name || ' ' || email)`.

Backward-compatibility and rollout
- Frontend currently falls back to mock users if `/api/users` returns 404. To minimize friction, implement the endpoint and return 200 + empty array rather than 404 during rollout.
- Consider feature-flagging access on the frontend (admin setting) to make rollout gradual.

Observability and testing
- Add integration tests:
  - response shape (fields present)
  - `q` search correctness
  - pagination headers and limits
  - authorization (401/403 for insufficient permissions)
- Add contract tests to ensure the OpenAPI remains consistent with implementation.

OpenAPI and client generation steps
1. Add the `/users` path to the OpenAPI spec (already drafted in repo). Use the `Users` tag.
2. Regenerate client/types if you use a codegen (e.g., OpenAPI generator / zod generation). For frontend, run the existing generator `node scripts/generate-zod.js` (or follow repo-specific instructions) so `src/types/api.d.ts` and any generated clients update.
3. Deploy backend and run frontend dev to confirm `GET /api/users` responds 200 via the Vite proxy.

Acceptance criteria
- `GET /users` returns 200 with array of `User` objects and `X-Total-Count` header when the endpoint is called with valid admin credentials.
- `GET /users?q=alice` returns users whose name/email match "alice".
- Requests without admin scope return 403.
- Pagination works and `per_page` respects the max limit.
- Frontend `AccessFlowPage` shows actual users in the left column when the endpoint is present and accessible.

Rollout plan
1. Implement endpoint in backend and add to OpenAPI.
2. Deploy to staging; run frontend against staging to verify the Access Flow loads users.
3. Enable endpoint in production behind admin-only RBAC.
4. Monitor logs and error rates for the first 24-72 hours.

Questions for backend implementers
- Do we want to expose all users to admins, or provide a server-side search-only API (i.e., require `q` param)? For large orgs a search-only approach is safer.
- Any existing auth scopes/names for admin operations we should use (e.g., `admin`, `users:read`)? Please confirm expected scope string so the frontend can align.

Contact
- Frontend owner: `@sukrokucing` (repo `scurve-fe`) — I can update the frontend client and regenerate types once the OpenAPI is published.

---

If you want, I can also:
- Draft a small server handler for your tech stack (Rust/Go/Python/Node) and add unit/integration tests.
- Add a small `User search` UI patch to `AccessFlowPage` so the frontend uses `GET /users?q=` instead of listing all users by default.
