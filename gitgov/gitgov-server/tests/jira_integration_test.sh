#!/bin/bash
# GitGov Jira Integration Test (V1.2-B Preview)
# Verifica: status, ingest válido, auth/secret, correlate, ticket-coverage

set -e

SERVER_URL="${SERVER_URL:-http://localhost:3000}"
API_KEY="${API_KEY:-57f1ed59-371d-46ef-9fdf-508f59bc4963}"
JIRA_SECRET="${JIRA_SECRET:-}"
TICKET_ID="${TICKET_ID:-PROJ-123}"
REPO_FULL_NAME="${REPO_FULL_NAME:-MapfrePE/GitGov}"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; exit 1; }
info() { echo -e "${YELLOW}- $1${NC}"; }

check_health() {
  info "Health check"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" "$SERVER_URL/health")
  [ "$code" = "200" ] || fail "Health check failed ($code)"
  pass "Server healthy"
}

check_status_endpoint() {
  info "Jira status endpoint (admin)"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $API_KEY" \
    "$SERVER_URL/integrations/jira/status")
  [ "$code" = "200" ] || fail "Jira status endpoint failed ($code)"
  pass "Jira status endpoint OK"
}

build_jira_payload() {
  local ts_ms="$1"
  cat <<EOF
{
  "webhookEvent": "jira:issue_updated",
  "timestamp": $ts_ms,
  "issue": {
    "key": "$TICKET_ID",
    "self": "https://example.atlassian.net/rest/api/2/issue/10001",
    "fields": {
      "summary": "Implementar $TICKET_ID en dashboard",
      "status": { "name": "In Progress" },
      "issuetype": { "name": "Task" },
      "priority": { "name": "Medium" },
      "assignee": { "displayName": "MapfrePE" },
      "reporter": { "displayName": "MapfrePE" },
      "created": "2026-02-24T20:10:00.000+0000",
      "updated": "2026-02-24T22:10:00.000+0000"
    }
  }
}
EOF
}

test_valid_ingest() {
  info "Ingesta Jira válida"
  local ts response
  ts=$(date +%s%3N)
  response=$(curl -s -X POST "$SERVER_URL/integrations/jira" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    ${JIRA_SECRET:+-H "x-gitgov-jira-secret: $JIRA_SECRET"} \
    -d "$(build_jira_payload "$ts")")

  echo "$response" | grep -q '"accepted":true' || fail "Ingesta Jira falló: $response"
  pass "Ingesta Jira aceptada"
}

test_auth_failure() {
  info "Auth inválida Jira (debe fallar)"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$SERVER_URL/integrations/jira" \
    -H "Authorization: Bearer invalid-key" \
    -H "Content-Type: application/json" \
    -d "$(build_jira_payload "$(date +%s%3N)")")

  [ "$code" = "401" ] || [ "$code" = "403" ] || fail "Se esperaba 401/403, llegó $code"
  pass "Auth inválida Jira rechazada ($code)"
}

test_secret_failure_if_enabled() {
  if [ -z "$JIRA_SECRET" ]; then
    info "Saltando test de secret inválido (JIRA_SECRET no configurado)"
    return 0
  fi

  info "Secret Jira inválido (debe fallar)"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$SERVER_URL/integrations/jira" \
    -H "Authorization: Bearer $API_KEY" \
    -H "x-gitgov-jira-secret: wrong-secret" \
    -H "Content-Type: application/json" \
    -d "$(build_jira_payload "$(date +%s%3N)")")

  [ "$code" = "401" ] || fail "Se esperaba 401 por secret inválido, llegó $code"
  pass "Secret Jira inválido rechazado"
}

test_correlate_endpoint() {
  info "Correlación Jira batch"
  local response
  response=$(curl -s -X POST "$SERVER_URL/integrations/jira/correlate" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"hours\":72,\"limit\":500,\"repo_full_name\":\"$REPO_FULL_NAME\"}")

  echo "$response" | grep -q '"scanned_commits"' || fail "Correlate endpoint falló: $response"
  pass "Correlate endpoint responde"
}

test_ticket_coverage_endpoint() {
  info "Ticket coverage endpoint"
  local response
  response=$(curl -s \
    -H "Authorization: Bearer $API_KEY" \
    "$SERVER_URL/integrations/jira/ticket-coverage?hours=72&repo_full_name=$REPO_FULL_NAME")

  echo "$response" | grep -q '"coverage_percentage"' || fail "Ticket coverage endpoint falló: $response"
  pass "Ticket coverage endpoint responde"
}

main() {
  echo "========================================="
  echo "GitGov Jira Integration Test (V1.2-B Preview)"
  echo "========================================="
  echo "Server: $SERVER_URL"
  echo "Repo: $REPO_FULL_NAME"
  echo "Ticket: $TICKET_ID"
  echo "Jira secret header: ${JIRA_SECRET:+enabled}${JIRA_SECRET:-disabled}"
  echo ""

  check_health
  check_status_endpoint
  test_valid_ingest
  test_auth_failure
  test_secret_failure_if_enabled
  test_correlate_endpoint
  test_ticket_coverage_endpoint

  echo ""
  echo "========================================="
  echo "Jira integration tests complete"
  echo "========================================="
  echo "Validated:"
  echo "  ✓ /integrations/jira/status (admin)"
  echo "  ✓ Jira ingest valid payload"
  echo "  ✓ Auth rejection"
  echo "  ✓ Secret rejection (if enabled)"
  echo "  ✓ Correlate endpoint"
  echo "  ✓ Ticket coverage endpoint"
}

main "$@"

