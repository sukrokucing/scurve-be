# Backend Brief: User Preferences (Timezone + Locale + Hour Cycle)

## Goal

Let each user persist display preferences (timezone, locale, 12/24h format) so that:
- FE renders dates/times in the user's timezone and locale
- Server-generated artifacts (email digests, exported reports, system-sent notifications, audit-log formatters) match the same preference
- Preference survives across devices/browsers (today FE only has localStorage)

FE default today: `timezone=Asia/Jakarta`, `locale=id-ID`, `hourCycle=24`. All backend timestamps remain UTC ISO-8601.

---

## Requested endpoints

### 1. `GET /auth/me/preferences`

Return the current user's preferences. If user has not saved any, return system defaults (never 404).

**Response 200**

```json
{
  "timezone": "Asia/Jakarta",
  "locale": "id-ID",
  "hour_cycle": 24,
  "updated_at": "2026-04-18T13:42:00Z"
}
```

**Field contract**

| Field        | Type     | Required | Notes                                                                                     |
|--------------|----------|----------|-------------------------------------------------------------------------------------------|
| `timezone`   | string   | yes      | IANA tz name (e.g. `Asia/Jakarta`). Validate against `zoneinfo` / ICU; reject bad values. |
| `locale`     | string   | yes      | BCP-47 tag (e.g. `id-ID`, `en-US`). Validate.                                             |
| `hour_cycle` | integer  | yes      | Enum: `12` or `24`. Reject other values.                                                  |
| `updated_at` | datetime | yes      | UTC ISO-8601. Set by server on write.                                                     |

**Defaults when unset** — same as FE fallback: `Asia/Jakarta` / `id-ID` / `24`.

**Caching** — `Cache-Control: private, max-age=60` + ETag. FE will send `If-None-Match`.

---

### 2. `PUT /auth/me/preferences`

Update preferences. Partial patch allowed via PATCH if you prefer; PUT replaces all three fields.

**Request body**

```json
{
  "timezone": "Asia/Makassar",
  "locale": "en-US",
  "hour_cycle": 24
}
```

**Validation**

- `timezone`: must exist in IANA tz DB (try `zoneinfo`/ICU lookup before accepting).
- `locale`: must parse as BCP-47 via `Intl.Locale` or equivalent library; accept any region/lang combo.
- `hour_cycle`: integer `12` or `24` only.
- Reject with `400` + standard error envelope (`{ error, message, fields? }`) on invalid fields. Specify which field failed in `fields`.

**Response 200** — same shape as GET.

**Audit** — emit `user.preferences.updated` audit log entry with `actor_id`, `changed_fields`.

---

### 3. Include preferences in `/auth/me` and `/auth/login`

Avoid one extra round-trip after login. Return preferences in the existing `me` / `login` payload under a `preferences` key:

```json
{
  "id": "...",
  "name": "Jimmy",
  "email": "jimmy@dwp.co.id",
  "preferences": {
    "timezone": "Asia/Jakarta",
    "locale": "id-ID",
    "hour_cycle": 24
  }
}
```

FE will hydrate its Zustand preferences store from that payload on login, then fall back to localStorage, then to built-in defaults.

---

## Behavior notes

- **All timestamps stay UTC**. Backend continues returning `created_at`, `updated_at`, notification timestamps, audit timestamps etc. as UTC ISO-8601. FE handles display conversion.
- **Server-side formatters** (email templates, exports, digest subjects like `"Daily summary — Apr 18"`) should honor the stored `timezone` + `locale` + `hour_cycle`. Use `zoneinfo`/`babel` (Python) or `date-fns-tz`/`Intl` (Node).
- **Calendar weeks** — if any week-start logic (week rollups, "this week"), use `locale` to decide Sun/Mon week-start, or add explicit `week_start: "sunday"|"monday"` field later (not for v1).
- **Scheduled jobs** — cron-style schedules can remain in UTC; the job payload can still be rendered per user timezone when sending output to user.

---

## OpenAPI sketch (v1)

```yaml
paths:
  /auth/me/preferences:
    get:
      summary: Get current user's display preferences
      responses:
        "200":
          content:
            application/json:
              schema: { $ref: "#/components/schemas/UserPreferences" }
    put:
      summary: Replace current user's display preferences
      requestBody:
        content:
          application/json:
            schema: { $ref: "#/components/schemas/UserPreferencesUpdate" }
      responses:
        "200":
          content:
            application/json:
              schema: { $ref: "#/components/schemas/UserPreferences" }
        "400":
          $ref: "#/components/responses/AppError"
components:
  schemas:
    UserPreferences:
      type: object
      required: [timezone, locale, hour_cycle, updated_at]
      properties:
        timezone:  { type: string, example: "Asia/Jakarta" }
        locale:    { type: string, example: "id-ID" }
        hour_cycle: { type: integer, enum: [12, 24] }
        updated_at: { type: string, format: date-time }
    UserPreferencesUpdate:
      type: object
      required: [timezone, locale, hour_cycle]
      properties:
        timezone:  { type: string }
        locale:    { type: string }
        hour_cycle: { type: integer, enum: [12, 24] }
```

---

## FE rollout plan

| Step | Owner | State                                                                 |
|------|-------|-----------------------------------------------------------------------|
| 1    | FE    | `usePreferencesStore` w/ localStorage + defaults `Asia/Jakarta`/`id-ID`/`24` ✅ |
| 2    | FE    | `DateTimePicker` tz-aware, tz offset hint in popover ✅               |
| 3    | FE    | `/settings/preferences` page ✅                                       |
| 4    | BE    | Implement GET/PUT /auth/me/preferences                                |
| 5    | BE    | Embed `preferences` in `/auth/me` + login payload                     |
| 6    | FE    | Swap localStorage hydration → API hydration on login                  |
| 7    | FE    | Migrate all date display to `formatInUserLocale` / `formatInUserZone` |
| 8    | BE    | Honor user preferences in email + export pipelines                    |

---

## Acceptance criteria

- `GET /auth/me/preferences` returns defaults (never 404) for users who never set preferences.
- `PUT` with bad `timezone` (e.g. `"Mars/Olympus"`) returns 400 with `fields.timezone`.
- `PUT` with `hour_cycle=13` returns 400.
- Updating preferences writes audit log `user.preferences.updated`.
- `/auth/me` payload includes `preferences` field.
- Email digest subject renders in user's timezone + locale once BE adopts.
