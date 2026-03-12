#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-https://127.0.0.1:8800}"
INSECURE_TLS="${INSECURE_TLS:-1}"
CLEANUP_PROJECT="${CLEANUP_PROJECT:-1}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd jq

CURL_FLAGS=("-sS")
if [[ "$INSECURE_TLS" != "0" ]]; then
  CURL_FLAGS+=("-k")
fi

LAST_BODY=""
LAST_STATUS=""

request() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  local token="${4:-}"
  local tmp
  tmp="$(mktemp)"

  local -a cmd=(curl "${CURL_FLAGS[@]}" -o "$tmp" -w "%{http_code}" -X "$method" "${BASE_URL}${path}" -H "Content-Type: application/json")

  if [[ -n "$token" ]]; then
    cmd+=( -H "Authorization: Bearer ${token}" )
  fi
  if [[ -n "$body" ]]; then
    cmd+=( -d "$body" )
  fi

  local status
  status="$("${cmd[@]}")"
  LAST_BODY="$(cat "$tmp")"
  LAST_STATUS="$status"
  rm -f "$tmp"
}

expect_status_any() {
  local status="$1"
  local label="$2"
  shift 2
  local expected
  for expected in "$@"; do
    if [[ "$status" == "$expected" ]]; then
      return 0
    fi
  done

  echo "[FAIL] $label -> expected one of: $* got: $status" >&2
  if [[ -n "$LAST_BODY" ]]; then
    echo "$LAST_BODY" | jq . 2>/dev/null >&2 || echo "$LAST_BODY" >&2
  fi
  exit 1
}

expect_status() {
  local status="$1"
  local expected="$2"
  local label="$3"
  if [[ "$status" != "$expected" ]]; then
    echo "[FAIL] $label -> expected: $expected got: $status" >&2
    if [[ -n "$LAST_BODY" ]]; then
      echo "$LAST_BODY" | jq . 2>/dev/null >&2 || echo "$LAST_BODY" >&2
    fi
    exit 1
  fi
}

suffix="$(date +%s)-$RANDOM"
email="smoke-${suffix}@example.com"
password="Passw0rd!${suffix}"
name="Smoke ${suffix}"

register_payload="$(jq -nc --arg n "$name" --arg e "$email" --arg p "$password" '{name:$n, email:$e, password:$p}')"
request POST "/auth/register" "$register_payload"
status="$LAST_STATUS"
expect_status_any "$status" "register" 200 201

token="$(printf '%s' "$LAST_BODY" | jq -r '.token // empty')"
if [[ -z "$token" ]]; then
  echo "[FAIL] register -> token missing" >&2
  echo "$LAST_BODY" | jq . >&2 || true
  exit 1
fi

echo "[OK] register ($status)"

request GET "/auth/me" "" "$token"
status="$LAST_STATUS"
expect_status "$status" "200" "auth/me"
user_id="$(printf '%s' "$LAST_BODY" | jq -r '.id // empty')"
if [[ -z "$user_id" ]]; then
  echo "[FAIL] auth/me -> id missing" >&2
  echo "$LAST_BODY" | jq . >&2 || true
  exit 1
fi

echo "[OK] auth/me"

project_payload="$(jq -nc --arg n "Smoke Project ${suffix}" --arg d "API smoke run" '{name:$n, description:$d}')"
request POST "/projects" "$project_payload" "$token"
status="$LAST_STATUS"
expect_status "$status" "201" "create project"
project_id="$(printf '%s' "$LAST_BODY" | jq -r '.id // empty')"
if [[ -z "$project_id" ]]; then
  echo "[FAIL] create project -> id missing" >&2
  echo "$LAST_BODY" | jq . >&2 || true
  exit 1
fi

echo "[OK] create project"

request GET "/projects/${project_id}/members" "" "$token"
status="$LAST_STATUS"
expect_status "$status" "200" "list project members"
resource_role_id="$(printf '%s' "$LAST_BODY" | jq -r --arg uid "$user_id" '
  (if type == "array" then . else (.members // []) end)
  | map(select(.user_id == $uid))
  | .[0].resource_roles[0].id // empty
')"
if [[ -z "$resource_role_id" ]]; then
  echo "[FAIL] list members -> no assigned resource role found for $user_id" >&2
  echo "$LAST_BODY" | jq . >&2 || true
  exit 1
fi

echo "[OK] list members (resolved role: $resource_role_id)"

task_payload="$(jq -nc --arg t "Smoke Task ${suffix}" '{title:$t}')"
request POST "/projects/${project_id}/tasks" "$task_payload" "$token"
status="$LAST_STATUS"
expect_status "$status" "201" "create task"
task_id="$(printf '%s' "$LAST_BODY" | jq -r '.id // empty')"
if [[ -z "$task_id" ]]; then
  echo "[FAIL] create task -> id missing" >&2
  echo "$LAST_BODY" | jq . >&2 || true
  exit 1
fi

echo "[OK] create task"

work_date="$(date -u +%F)"
work_log_payload="$(jq -nc --arg rr "$resource_role_id" --arg wd "$work_date" '{resource_role_id:$rr, hours:2.5, work_date:$wd, note:"smoke"}')"
request POST "/projects/${project_id}/tasks/${task_id}/work-logs" "$work_log_payload" "$token"
status="$LAST_STATUS"
expect_status "$status" "201" "create work-log"

echo "[OK] create work-log"

for metric in progress hours cost; do
  request GET "/projects/${project_id}/s-curve/health?metric=${metric}" "" "$token"
  status="$LAST_STATUS"
  expect_status "$status" "200" "health metric=${metric}"
  echo "[OK] s-curve health metric=${metric}"
done

for metric in progress hours cost; do
  request GET "/projects/${project_id}/dashboard?metric=${metric}" "" "$token"
  status="$LAST_STATUS"
  expect_status "$status" "200" "dashboard metric=${metric}"
  echo "[OK] dashboard metric=${metric}"
done

request GET "/portfolio/s-curve/summary?metric=hours" "" "$token"
status="$LAST_STATUS"
expect_status "$status" "200" "portfolio summary hours"
echo "[OK] portfolio summary metric=hours"

if [[ "$CLEANUP_PROJECT" != "0" ]]; then
  request DELETE "/projects/${project_id}" "" "$token"
  status="$LAST_STATUS"
  if [[ "$status" == "200" || "$status" == "204" ]]; then
    echo "[OK] cleanup project"
  else
    echo "[WARN] cleanup project skipped (status $status)"
  fi
fi

echo "[PASS] smoke API checks completed"
