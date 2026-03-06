#!/bin/bash
# GitGov Jenkins Integration Test (V1.2-A)
# Verifica: health, status endpoint, ingest válido, duplicado, auth/secret

set -e

SERVER_URL="${SERVER_URL:-http://localhost:3000}"
API_KEY="${API_KEY:-${GITGOV_API_KEY:-}}"
JENKINS_SECRET="${JENKINS_SECRET:-}"
REPO_FULL_NAME="${REPO_FULL_NAME:-MapfrePE/GitGov}"
BRANCH="${BRANCH:-main}"
COMMIT_SHA="${COMMIT_SHA:-abc123def4567890}"

if [ -z "$API_KEY" ]; then
  echo "✗ Missing API key. Set API_KEY or GITGOV_API_KEY."
  exit 1
fi

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}✓ $1${NC}"; }
fail() { echo -e "${RED}✗ $1${NC}"; exit 1; }
info() { echo -e "${YELLOW}- $1${NC}"; }

auth_headers() {
  if [ -n "$JENKINS_SECRET" ]; then
    printf -- '-H\nAuthorization: Bearer %s\n-H\nx-gitgov-jenkins-secret: %s\n' "$API_KEY" "$JENKINS_SECRET"
  else
    printf -- '-H\nAuthorization: Bearer %s\n' "$API_KEY"
  fi
}

check_health() {
  info "Health check"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" "$SERVER_URL/health")
  [ "$code" = "200" ] || fail "Health check failed ($code)"
  pass "Server healthy"
}

check_status_endpoint() {
  info "Jenkins status endpoint (admin)"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $API_KEY" \
    "$SERVER_URL/integrations/jenkins/status")
  [ "$code" = "200" ] || fail "Status endpoint failed ($code)"
  pass "Jenkins status endpoint OK"
}

build_payload() {
  local pipeline_id="$1"
  local ts="$2"
  cat <<EOF
{
  "pipeline_id": "$pipeline_id",
  "job_name": "gitgov/main",
  "status": "success",
  "commit_sha": "$COMMIT_SHA",
  "branch": "$BRANCH",
  "repo_full_name": "$REPO_FULL_NAME",
  "duration_ms": 45000,
  "triggered_by": "MapfrePE",
  "stages": [
    { "name": "Build", "status": "success", "duration_ms": 12000 },
    { "name": "Test", "status": "success", "duration_ms": 28000 }
  ],
  "artifacts": ["gitgov-server-v1.2.0.tar.gz"],
  "timestamp": $ts
}
EOF
}

test_valid_ingest() {
  info "Ingesta Jenkins válida"
  local ts pipeline_id response
  ts=$(date +%s%3N)
  pipeline_id="build-$ts"
  response=$(curl -s -X POST "$SERVER_URL/integrations/jenkins" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    ${JENKINS_SECRET:+-H "x-gitgov-jenkins-secret: $JENKINS_SECRET"} \
    -d "$(build_payload "$pipeline_id" "$ts")")

  echo "$response" | grep -q '"accepted":true' || fail "Ingesta válida falló: $response"
  pass "Ingesta válida aceptada"

  LAST_PIPELINE_ID="$pipeline_id"
  LAST_TS="$ts"
}

test_duplicate_ingest() {
  info "Detección de duplicado Jenkins"
  local response
  response=$(curl -s -X POST "$SERVER_URL/integrations/jenkins" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $API_KEY" \
    ${JENKINS_SECRET:+-H "x-gitgov-jenkins-secret: $JENKINS_SECRET"} \
    -d "$(build_payload "$LAST_PIPELINE_ID" "$LAST_TS")")

  echo "$response" | grep -q '"duplicate":true' || fail "Duplicado no detectado: $response"
  pass "Duplicado detectado"
}

test_auth_failure() {
  info "Auth inválida (debe fallar)"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$SERVER_URL/integrations/jenkins" \
    -H "Authorization: Bearer invalid-key" \
    -H "Content-Type: application/json" \
    -d "$(build_payload "bad-auth" "$(date +%s%3N)")")

  [ "$code" = "401" ] || [ "$code" = "403" ] || fail "Se esperaba 401/403, llegó $code"
  pass "Auth inválida rechazada ($code)"
}

test_secret_failure_if_enabled() {
  if [ -z "$JENKINS_SECRET" ]; then
    info "Saltando test de secret inválido (JENKINS_SECRET no configurado)"
    return 0
  fi

  info "Secret Jenkins inválido (debe fallar)"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "$SERVER_URL/integrations/jenkins" \
    -H "Authorization: Bearer $API_KEY" \
    -H "x-gitgov-jenkins-secret: wrong-secret" \
    -H "Content-Type: application/json" \
    -d "$(build_payload "bad-secret" "$(date +%s%3N)")")

  [ "$code" = "401" ] || fail "Se esperaba 401 por secret inválido, llegó $code"
  pass "Secret inválido rechazado"
}

test_correlations_endpoint() {
  info "Correlations endpoint (admin)"
  local response
  response=$(curl -s \
    -H "Authorization: Bearer $API_KEY" \
    "$SERVER_URL/integrations/jenkins/correlations?limit=5&offset=0")
  echo "$response" | grep -q '"correlations"' || fail "Correlations endpoint falló: $response"
  pass "Correlations endpoint responde"
}

main() {
  echo "========================================="
  echo "GitGov Jenkins Integration Test (V1.2-A)"
  echo "========================================="
  echo "Server: $SERVER_URL"
  echo "Repo: $REPO_FULL_NAME"
  echo "Branch: $BRANCH"
  echo "Jenkins secret header: ${JENKINS_SECRET:+enabled}${JENKINS_SECRET:-disabled}"
  echo ""

  check_health
  check_status_endpoint
  test_valid_ingest
  test_duplicate_ingest
  test_auth_failure
  test_secret_failure_if_enabled
  test_correlations_endpoint

  echo ""
  echo "========================================="
  echo "Jenkins integration tests complete"
  echo "========================================="
  echo "Validated:"
  echo "  ✓ /integrations/jenkins/status (admin)"
  echo "  ✓ Jenkins ingest valid payload"
  echo "  ✓ Duplicate detection"
  echo "  ✓ Auth rejection"
  echo "  ✓ Secret rejection (if enabled)"
  echo "  ✓ Correlations endpoint"
}

main "$@"
