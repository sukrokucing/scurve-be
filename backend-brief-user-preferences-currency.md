# Backend Brief: Add `currency` to User Preferences

## Goal

Extend the existing `user_preferences` feature with a 4th field — `currency` — so users control how monetary amounts render across the app (project budgets, work-log cost estimates, exports, invoices).

FE default: `IDR`. Backend defaults should match.

---

## Change surface

### 1. Schema additions

Extend `UserPreferences` + `UserPreferencesUpdate` + `UserPreferencesEmbed` with a required `currency` field.

```yaml
UserPreferences:
  type: object
  required: [timezone, locale, hour_cycle, currency, updated_at]
  properties:
    timezone:   { type: string, example: "Asia/Jakarta" }
    locale:     { type: string, example: "id-ID" }
    hour_cycle: { type: integer, enum: [12, 24] }
    currency:   { type: string, example: "IDR", description: "ISO 4217 3-letter currency code, uppercase" }
    updated_at: { type: string, format: date-time }

UserPreferencesUpdate:
  type: object
  required: [timezone, locale, hour_cycle, currency]
  properties:
    timezone:   { type: string }
    locale:     { type: string }
    hour_cycle: { type: integer, enum: [12, 24] }
    currency:   { type: string }

UserPreferencesEmbed:
  # Same new field, omit updated_at as before
  required: [timezone, locale, hour_cycle, currency]
```

Default when user has no saved preferences — return `"IDR"`.

### 2. `PUT /auth/me/preferences` validation

- `currency` — required, 3 uppercase alpha chars matching `^[A-Z]{3}$`
- Must be in an ISO 4217 allowlist. Use a library (Python: `babel.numbers.list_currencies()`; Go: `golang.org/x/text/currency`; Node: `Intl.supportedValuesOf('currency')`). Reject unknown codes.
- Accept mixed case but normalize to uppercase on write.
- Reject with 400 using same envelope pattern:

```json
{ "error": "bad_request", "message": "invalid currency 'XYZ' (field: currency)" }
```

FE already parses `(field: X)` so keep that suffix convention.

### 3. `GET /auth/me/preferences`

Include `currency` in response. Never return null — fall back to server default (`IDR`).

### 4. Login / Register / Me payload embed

`POST /auth/login`, `POST /auth/register`, `GET /auth/me` payloads already embed `preferences`. Add `currency` inside:

```json
{
  "preferences": {
    "timezone": "Asia/Jakarta",
    "locale": "id-ID",
    "hour_cycle": 24,
    "currency": "IDR"
  }
}
```

### 5. Migration

- DB column: `user_preferences.currency VARCHAR(3) NOT NULL DEFAULT 'IDR'`
- Backfill existing rows to `IDR`.
- Emit audit `user.preferences.updated` same as other fields; `changed_fields` includes `currency`.

### 6. Server-side formatters (rollout step 8, separate ticket but mentioning early)

When BE renders monetary amounts in user-facing artifacts (email digests, PDF exports, CSV exports, dashboards sent via webhook), format using `user.preferences.currency` + `user.preferences.locale`:

- Python: `babel.numbers.format_currency(amount, currency, locale=locale)`
- Go: `x/text/currency` + `message.NewPrinter(lang)`
- Node: `new Intl.NumberFormat(locale, { style: 'currency', currency })`

**Important** — per-project currency still takes precedence. If a project stores its own currency (`projects.currency`), render that project's amounts in the project currency, not the user's. User currency is the **fallback** for amounts without a project context (e.g. portfolio totals, cross-project summaries).

---

## Acceptance criteria

- `GET /auth/me/preferences` returns `currency: "IDR"` for users who never saved prefs.
- `PUT` with `currency: "USD"` returns 200 and persists.
- `PUT` with `currency: "usd"` normalizes to `"USD"` on write.
- `PUT` with `currency: "XYZ"` returns 400 with message matching `(field: currency)` suffix.
- `PUT` missing `currency` returns 400 (required).
- `/auth/me`, `/auth/login`, `/auth/register` responses include `preferences.currency`.
- Audit log entry on update lists `currency` in `changed_fields` when changed.
- OpenAPI regenerated and published at `/docs`.

---

## Non-goals (this ticket)

- Currency conversion / FX rates. We only store + format. Conversion is out of scope.
- Per-organization default currency. User pref is personal only.
- Currency symbol customization. We rely on `Intl.NumberFormat` locale rules.

---

## FE ready-state

FE changes already in place behind the new `currency` field:

- `usePreferencesStore` carries `currency` + `setCurrency` + hydrates from API
- `formatCurrency(amount)` helper uses the preferred currency + locale
- `/settings/preferences` page has Currency Combobox with 15 common ISO codes + sample "1,234,567.89" preview
- Default `IDR` client-side; gracefully falls back if BE returns unknown code

Once BE ships, FE `PUT` body will include `currency` and user selection round-trips. Until then, `PUT` without `currency` would 400 per validation — so deploy together.

---

## Suggested request envelope

```http
PUT /auth/me/preferences
Content-Type: application/json
Authorization: Bearer <token>

{
  "timezone": "Asia/Jakarta",
  "locale": "id-ID",
  "hour_cycle": 24,
  "currency": "IDR"
}
```

Response 200:

```json
{
  "timezone": "Asia/Jakarta",
  "locale": "id-ID",
  "hour_cycle": 24,
  "currency": "IDR",
  "updated_at": "2026-04-19T01:00:00Z"
}
```
