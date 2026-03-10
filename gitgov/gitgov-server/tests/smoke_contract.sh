#!/bin/bash
# smoke_contract.sh — Contract smoke test for GitGov API
#
# Sections:
#   A) Pagination defaults — endpoints work without offset/limit
#   B) Golden Path — stage_files → commit → attempt_push → successful_push
#      are accepted by /events, and all four appear in /logs
#
# Usage:
#   SERVER_URL=http://127.0.0.1:3000 API_KEY=<key> bash smoke_contract.sh
#
# Exit 0 = all checks pass, Exit 1 = one or more failed.

SERVER_URL="${SERVER_URL:-http://127.0.0.1:3000}"
API_KEY="${API_KEY:-${GITGOV_API_KEY:-}}"

if [ -z "$API_KEY" ]; then
  echo "❌ Missing API key. Set API_KEY or GITGOV_API_KEY."
  exit 1
fi

FAILED=0
PASS=0

pass() { echo "✅ $1"; PASS=$((PASS + 1)); }
fail() { echo "❌ $1"; FAILED=$((FAILED + 1)); }
AUTH_HEADER="Authorization: Bearer $API_KEY"

echo "========================================"
echo "GitGov API Smoke / Contract Test"
echo "========================================"
echo "Server: $SERVER_URL"
echo ""

# ── UUID helper ──────────────────────────────────────────────────────────────
new_uuid() {
  local u=""
  u=$(uuidgen 2>/dev/null | tr '[:upper:]' '[:lower:]' | tr -d '\r\n' || true)
  if [ -n "$u" ]; then echo "$u"; return 0; fi
  u=$(powershell.exe -NoProfile -Command "[System.Guid]::NewGuid().ToString().ToLowerInvariant()" 2>/dev/null | tr -d '\r\n' || true)
  if [ -n "$u" ]; then echo "$u"; return 0; fi
  u=$(python - <<'PY' 2>/dev/null || true
import uuid
print(str(uuid.uuid4()))
PY
)
  u=$(echo "$u" | tr -d '\r\n')
  if [ -n "$u" ]; then echo "$u"; return 0; fi
  cat /proc/sys/kernel/random/uuid 2>/dev/null | tr -d '\r\n' || true
}

echo "── Section A: Pagination defaults ──────────────────────────────────────"

# 1. Health — public, no auth
CODE=$(curl -s -o /dev/null -w "%{http_code}" "$SERVER_URL/health")
[ "$CODE" = "200" ] && pass "/health → 200" || fail "/health → $CODE (expected 200)"

# 2. /logs without offset (regression: was failing with 'missing field offset')
RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/logs?limit=5")
echo "$RES" | grep -q '"events"' \
  && pass "/logs?limit=5 (no offset) → has 'events' field" \
  || fail "/logs?limit=5 (no offset) → unexpected: ${RES:0:120}"

# 3. /logs with explicit offset — backward compat
RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/logs?limit=5&offset=0")
echo "$RES" | grep -q '"events"' \
  && pass "/logs?limit=5&offset=0 → has 'events' field" \
  || fail "/logs?limit=5&offset=0 → unexpected: ${RES:0:120}"

# 4. /integrations/jenkins/correlations without offset (regression)
RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/integrations/jenkins/correlations?limit=5")
echo "$RES" | grep -q '"correlations"' \
  && pass "/integrations/jenkins/correlations?limit=5 (no offset) → has 'correlations' field" \
  || fail "/integrations/jenkins/correlations?limit=5 (no offset) → unexpected: ${RES:0:120}"

# 5. /integrations/jenkins/correlations with explicit offset — backward compat
RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/integrations/jenkins/correlations?limit=5&offset=0")
echo "$RES" | grep -q '"correlations"' \
  && pass "/integrations/jenkins/correlations?limit=5&offset=0 → has 'correlations' field" \
  || fail "/integrations/jenkins/correlations?limit=5&offset=0 → unexpected: ${RES:0:120}"

# 6. /signals without offset
RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/signals?limit=5")
echo "$RES" | grep -q '"signals"' \
  && pass "/signals?limit=5 (no offset) → has 'signals' field" \
  || fail "/signals?limit=5 (no offset) → unexpected: ${RES:0:120}"

# 7. /governance-events without offset
RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/governance-events?limit=5")
echo "$RES" | grep -q '"events"' \
  && pass "/governance-events?limit=5 (no offset) → has 'events' field" \
  || fail "/governance-events?limit=5 (no offset) → unexpected: ${RES:0:120}"

# 8. /logs without ANY params — defaults must kick in
RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/logs")
echo "$RES" | grep -q '"events"' \
  && pass "/logs (no params at all) → has 'events' field" \
  || fail "/logs (no params at all) → unexpected: ${RES:0:120}"

echo ""
echo "── Section B: Golden Path ───────────────────────────────────────────────"
echo "   stage_files → commit → attempt_push → successful_push"
echo ""

TS=$(date +%s%3N 2>/dev/null || date +%s)000
UUID_STAGE=$(new_uuid)
UUID_COMMIT=$(new_uuid)
UUID_PUSH_ATTEMPT=$(new_uuid)
UUID_PUSH_OK=$(new_uuid)
GP_BRANCH="feat/smoke-golden-path"
GP_USER="manual_check"
GP_REPO="MapfrePE/GitGov"

send_event() {
  local uuid="$1" etype="$2" extra="$3"
  curl -s -X POST "$SERVER_URL/events" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{
      \"events\": [{
        \"event_uuid\": \"$uuid\",
        \"event_type\": \"$etype\",
        \"user_login\": \"$GP_USER\",
        \"repo_full_name\": \"$GP_REPO\",
        \"branch\": \"$GP_BRANCH\",
        \"files\": [\"src/main.rs\"],
        \"status\": \"success\",
        \"timestamp\": $TS
        $extra
      }],
      \"client_version\": \"smoke-1.0\"
    }"
}

# GP-1: stage_files
RES=$(send_event "$UUID_STAGE" "stage_files" "")
echo "$RES" | grep -q "\"$UUID_STAGE\"" \
  && pass "GP stage_files accepted (uuid in response)" \
  || fail "GP stage_files not accepted: ${RES:0:150}"

# GP-2: commit
RES=$(send_event "$UUID_COMMIT" "commit" ", \"commit_sha\": \"deadbeef12345678\"")
echo "$RES" | grep -q "\"$UUID_COMMIT\"" \
  && pass "GP commit accepted (uuid in response)" \
  || fail "GP commit not accepted: ${RES:0:150}"

# GP-3: attempt_push
RES=$(send_event "$UUID_PUSH_ATTEMPT" "attempt_push" "")
echo "$RES" | grep -q "\"$UUID_PUSH_ATTEMPT\"" \
  && pass "GP attempt_push accepted (uuid in response)" \
  || fail "GP attempt_push not accepted: ${RES:0:150}"

# GP-4: successful_push
RES=$(send_event "$UUID_PUSH_OK" "successful_push" "")
echo "$RES" | grep -q "\"$UUID_PUSH_OK\"" \
  && pass "GP successful_push accepted (uuid in response)" \
  || fail "GP successful_push not accepted: ${RES:0:150}"

# GP-5: verify all 4 appear in /logs
sleep 1
LOGS=$(curl -s -H "Authorization: Bearer $API_KEY" "$SERVER_URL/logs?limit=20")
for uuid in "$UUID_STAGE" "$UUID_COMMIT" "$UUID_PUSH_ATTEMPT" "$UUID_PUSH_OK"; do
  [ -z "$uuid" ] && fail "Generated UUID is empty (environment issue)"
  echo "$LOGS" | grep -q "$uuid" \
    && pass "GP event $uuid visible in /logs" \
    || fail "GP event $uuid NOT found in /logs"
done

# GP-6: duplicate rejection — re-send successful_push UUID, expect in duplicates[]
RES=$(send_event "$UUID_PUSH_OK" "successful_push" "")
echo "$RES" | grep -q "\"$UUID_PUSH_OK\"" \
  && echo "$RES" | grep -q '"duplicates"' \
  && pass "GP duplicate UUID correctly rejected (in duplicates[])" \
  || fail "GP duplicate not detected: ${RES:0:150}"

echo ""
echo "── Section C: Read-only extended checks (new endpoints) ─────────────────"

# C-1: GET /stats/daily?days=14 → HTTP 200 + JSON array with exactly 14 day entries
#   No migration required (uses client_events, core table since v1).
CODE=$(curl -s -o /dev/null -w "%{http_code}" -H "$AUTH_HEADER" "$SERVER_URL/stats/daily?days=14")
if [ "$CODE" = "200" ]; then
  RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/stats/daily?days=14")
  COUNT=$(echo "$RES" | grep -o '"day"' | wc -l | tr -d ' \r\n\t')
  [ "$COUNT" = "14" ] \
    && pass "/stats/daily?days=14 → HTTP 200, array with 14 day entries" \
    || fail "/stats/daily?days=14 → expected 14 day entries, got ${COUNT}: ${RES:0:150}"
else
  fail "/stats/daily?days=14 → HTTP $CODE (expected 200)"
fi

# C-2: GET /clients → HTTP 200 + has 'sessions' field
#   REQUIRES schema v8: client_sessions table (supabase_schema_v8.sql).
CODE=$(curl -s -o /dev/null -w "%{http_code}" -H "$AUTH_HEADER" "$SERVER_URL/clients")
if [ "$CODE" = "200" ]; then
  RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/clients")
  echo "$RES" | grep -q '"sessions"' \
    && pass "/clients → HTTP 200, has 'sessions' field" \
    || fail "/clients → HTTP 200 but no 'sessions' field: ${RES:0:150}"
else
  fail "/clients → HTTP $CODE (expected 200) — MIGRATION REQUIRED: apply supabase_schema_v8.sql (client_sessions table)"
fi

# C-3: GET /identities/aliases → HTTP 200 + valid JSON array
#   REQUIRES schema v8: identity_aliases table (supabase_schema_v8.sql).
CODE=$(curl -s -o /dev/null -w "%{http_code}" -H "$AUTH_HEADER" "$SERVER_URL/identities/aliases")
if [ "$CODE" = "200" ]; then
  RES=$(curl -s -H "$AUTH_HEADER" "$SERVER_URL/identities/aliases")
  if echo "$RES" | grep -q '"error"'; then
    fail "/identities/aliases → HTTP 200 but body contains error — MIGRATION REQUIRED: apply supabase_schema_v8.sql: ${RES:0:150}"
  else
    FIRST=$(echo "$RES" | tr -d ' \r\n' | cut -c1)
    [ "$FIRST" = "[" ] \
      && pass "/identities/aliases → HTTP 200, valid JSON array" \
      || fail "/identities/aliases → HTTP 200 but unexpected shape (not array): ${RES:0:150}"
  fi
else
  fail "/identities/aliases → HTTP $CODE (expected 200) — MIGRATION REQUIRED: apply supabase_schema_v8.sql (identity_aliases table)"
fi

echo ""
echo "========================================"
echo "Results: $PASS passed, $FAILED failed"
if [ "$FAILED" -eq 0 ]; then
  echo "Exit: 0 ✅  All contract checks passed."
  exit 0
else
  echo "Exit: 1 ❌  $FAILED check(s) failed — see output above."
  exit 1
fi
