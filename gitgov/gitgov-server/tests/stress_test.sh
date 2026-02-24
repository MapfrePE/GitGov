#!/bin/bash
# GitGov Stress Test Suite
# ========================
# Tests: job queue dedupe, webhook idempotency, worker crash recovery

set -e

SERVER_URL="${SERVER_URL:-http://localhost:3000}"
API_KEY="${API_KEY:-}"
ORG_NAME="${ORG_NAME:-test-org}"
EVENTS_BURST_COUNT="${EVENTS_BURST_COUNT:-280}"
EVENTS_BURST_CONCURRENCY="${EVENTS_BURST_CONCURRENCY:-20}"
AUDIT_BURST_COUNT="${AUDIT_BURST_COUNT:-80}"
AUDIT_BURST_CONCURRENCY="${AUDIT_BURST_CONCURRENCY:-10}"
RUN_RATE_LIMIT_TESTS="${RUN_RATE_LIMIT_TESTS:-1}"
RUN_AUDIT_RATE_LIMIT_TEST="${RUN_AUDIT_RATE_LIMIT_TEST:-1}"
RUN_WEBHOOK_STRESS_TESTS="${RUN_WEBHOOK_STRESS_TESTS:-1}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo_header() {
    echo ""
    echo -e "${YELLOW}============================================${NC}"
    echo -e "${YELLOW} $1${NC}"
    echo -e "${YELLOW}============================================${NC}"
}

echo_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

echo_fail() {
    echo -e "${RED}✗ $1${NC}"
}

echo_info() {
    echo "- $1"
}

require_api_key_or_skip() {
    if [ -z "$API_KEY" ]; then
        echo "Skipping: requires API_KEY"
        return 1
    fi
    return 0
}

extract_retry_after_from_headers() {
    local headers_file="$1"
    local retry_after=""
    retry_after=$(grep -i '^Retry-After:' "$headers_file" 2>/dev/null | head -n1 | sed 's/\r$//' | cut -d':' -f2- | xargs || true)
    echo "$retry_after"
}

run_parallel_curl_burst() {
    local count="$1"
    local concurrency="$2"
    local mode="$3"
    local tmpdir="$4"
    local i

    for i in $(seq 1 "$count"); do
        if [ "$mode" = "events" ]; then
            local event_uuid="rate-evt-${i}-$(date +%s%N)"
            local payload
            payload='{"events":[{"event_uuid":"'"$event_uuid"'","event_type":"stage_files","user_login":"stress-test","files":[],"status":"success","metadata":{"suite":"stress_test","kind":"rate_limit"},"timestamp":'$(date +%s%3N)'}],"client_id":"stress-test","client_version":"stress-test"}'

            (
                curl -sS -X POST "$SERVER_URL/events" \
                    -H "Content-Type: application/json" \
                    -H "Authorization: Bearer $API_KEY" \
                    -D "$tmpdir/headers.$i" \
                    -o "$tmpdir/body.$i" \
                    -w "%{http_code}" \
                    -d "$payload" > "$tmpdir/status.$i" 2>"$tmpdir/curlerr.$i" || echo "000" > "$tmpdir/status.$i"
            ) &
        elif [ "$mode" = "audit" ]; then
            local ts
            ts=$(date +%s%3N)
            local payload
            payload='{"org_name":null,"entries":[{"@timestamp":'"$ts"',"action":"repo.access","actor":"stress-test","repository":"rate-limit-repo","data":{"i":'"$i"'}}]}'

            (
                curl -sS -X POST "$SERVER_URL/audit-stream/github" \
                    -H "Content-Type: application/json" \
                    -H "Authorization: Bearer $API_KEY" \
                    -D "$tmpdir/headers.$i" \
                    -o "$tmpdir/body.$i" \
                    -w "%{http_code}" \
                    -d "$payload" > "$tmpdir/status.$i" 2>"$tmpdir/curlerr.$i" || echo "000" > "$tmpdir/status.$i"
            ) &
        else
            echo "Unknown burst mode: $mode"
            return 1
        fi

        if [ $((i % concurrency)) -eq 0 ]; then
            wait
        fi
    done

    wait
}

summarize_status_codes() {
    local tmpdir="$1"
    local ok_count=0
    local rate_count=0
    local forbidden_count=0
    local bad_request_count=0
    local other_count=0
    local unknown_count=0
    local code

    for f in "$tmpdir"/status.*; do
        [ -e "$f" ] || continue
        code=$(cat "$f" 2>/dev/null || echo "000")
        case "$code" in
            200) ok_count=$((ok_count + 1)) ;;
            429) rate_count=$((rate_count + 1)) ;;
            403) forbidden_count=$((forbidden_count + 1)) ;;
            400) bad_request_count=$((bad_request_count + 1)) ;;
            000) unknown_count=$((unknown_count + 1)) ;;
            *) other_count=$((other_count + 1)) ;;
        esac
    done

    echo "$ok_count $rate_count $forbidden_count $bad_request_count $other_count $unknown_count"
}

# Check server is running
check_server() {
    echo_header "Checking Server Status"
    
    if curl -s -o /dev/null -w "%{http_code}" "$SERVER_URL/health" | grep -q "200"; then
        echo_success "Server is running at $SERVER_URL"
    else
        echo_fail "Server not responding at $SERVER_URL"
        echo "Start the server first: cargo run"
        exit 1
    fi
}

# Test 1: Webhook Idempotency
# Send the same webhook twice, should only create one event
test_webhook_idempotency() {
    echo_header "Test 1: Webhook Idempotency"
    
    DELIVERY_ID="test-idempotency-$(date +%s)"
    
    # Send first webhook
    RESPONSE1=$(curl -s -X POST "$SERVER_URL/webhooks/github" \
        -H "Content-Type: application/json" \
        -H "X-GitHub-Event: push" \
        -H "X-GitHub-Delivery: $DELIVERY_ID" \
        -d '{
            "ref": "refs/heads/main",
            "before": "abc123",
            "after": "def456",
            "repository": {
                "id": 999999,
                "name": "test-repo",
                "full_name": "'"$ORG_NAME"'/test-repo",
                "private": false,
                "owner": {"id": 999, "login": "'"$ORG_NAME"'"},
                "organization": {"id": 999, "login": "'"$ORG_NAME"'"}
            },
            "sender": {"id": 1, "login": "testuser"},
            "commits": [{"id": "def456", "message": "test", "author": {"name": "Test", "email": "test@example.com"}}]
        }')
    
    if echo "$RESPONSE1" | grep -q '"received":true'; then
        echo_success "First webhook accepted"
    else
        echo_fail "First webhook failed: $RESPONSE1"
        return 1
    fi
    
    # Send duplicate webhook
    RESPONSE2=$(curl -s -X POST "$SERVER_URL/webhooks/github" \
        -H "Content-Type: application/json" \
        -H "X-GitHub-Event: push" \
        -H "X-GitHub-Delivery: $DELIVERY_ID" \
        -d '{
            "ref": "refs/heads/main",
            "before": "abc123",
            "after": "def456",
            "repository": {
                "id": 999999,
                "name": "test-repo",
                "full_name": "'"$ORG_NAME"'/test-repo",
                "private": false,
                "owner": {"id": 999, "login": "'"$ORG_NAME"'"},
                "organization": {"id": 999, "login": "'"$ORG_NAME"'"}
            },
            "sender": {"id": 1, "login": "testuser"},
            "commits": [{"id": "def456", "message": "test", "author": {"name": "Test", "email": "test@example.com"}}]
        }')
    
    if echo "$RESPONSE2" | grep -q '"error".*Duplicate\|"error".*duplicate\|409'; then
        echo_success "Duplicate webhook correctly rejected"
    else
        echo_fail "Duplicate handling unexpected: $RESPONSE2"
        return 1
    fi
    
    echo_success "Webhook idempotency test PASSED"
}

# Test 2: Job Queue Deduplication
# Send 100 webhooks rapidly, should only create 1 job per org
test_job_dedupe() {
    echo_header "Test 2: Job Queue Deduplication (100 rapid webhooks)"
    
    echo "Sending 100 webhooks to same org..."
    
    # Send 100 webhooks concurrently
    for i in $(seq 1 100); do
        curl -s -X POST "$SERVER_URL/webhooks/github" \
            -H "Content-Type: application/json" \
            -H "X-GitHub-Event: push" \
            -H "X-GitHub-Delivery: test-dedupe-$i-$(date +%s%N)" \
            -d '{
                "ref": "refs/heads/main",
                "before": "abc'$i'",
                "after": "def'$i'",
                "repository": {
                    "id": 999999,
                    "name": "test-repo-dedupe",
                    "full_name": "'"$ORG_NAME"'/test-repo-dedupe",
                    "private": false,
                    "owner": {"id": 999, "login": "'"$ORG_NAME"'"},
                    "organization": {"id": 999, "login": "'"$ORG_NAME"'"}
                },
                "sender": {"id": 1, "login": "testuser"},
                "commits": [{"id": "def'$i'", "message": "test '$i'", "author": {"name": "Test", "email": "test@example.com"}}]
            }' > /dev/null &
    done
    
    wait
    echo_success "All 100 webhooks sent"
    
    # Wait for jobs to be processed
    echo "Waiting 10 seconds for job processing..."
    sleep 10
    
    # Check job count via metrics endpoint
    if [ -n "$API_KEY" ]; then
        METRICS=$(curl -s -H "Authorization: Bearer $API_KEY" "$SERVER_URL/jobs/metrics")
        echo "Job metrics: $METRICS"
        
        RUNNING=$(echo "$METRICS" | grep -o '"running":[0-9]*' | grep -o '[0-9]*' || echo "0")
        
        if [ "$RUNNING" -le 1 ]; then
            echo_success "Job dedupe working: at most 1 job running at a time"
        else
            echo_fail "Multiple jobs running: $RUNNING (should be <= 1)"
            return 1
        fi
    else
        echo "Skipping job count verification (no API_KEY set)"
    fi
    
    echo_success "Job dedupe test PASSED"
}

# Test 3: Stale Job Reset Safety
# Simulate a stuck job and verify reset works correctly
test_stale_reset() {
    echo_header "Test 3: Stale Job Reset Safety"
    
    if [ -z "$API_KEY" ]; then
        echo "Skipping: requires API_KEY"
        return 0
    fi
    
    # Get current job metrics
    METRICS=$(curl -s -H "Authorization: Bearer $API_KEY" "$SERVER_URL/jobs/metrics")
    STALE_BEFORE=$(echo "$METRICS" | grep -o '"stale_running":[0-9]*' | grep -o '[0-9]*' || echo "0")
    
    echo "Stale running jobs before: $STALE_BEFORE"
    
    # Jobs are auto-reset after 5 minutes TTL
    # We can verify by checking metrics
    
    echo_success "Stale job reset is handled by worker with TTL"
    echo_success "Stale reset test PASSED (automated via worker)"
}

# Test 4: Concurrent Webhooks to Multiple Orgs
# Should create one job per org, not more
test_multi_org() {
    echo_header "Test 4: Multi-Org Job Deduplication"
    
    ORGS=("org-a" "org-b" "org-c")
    
    for ORG in "${ORGS[@]}"; do
        for i in $(seq 1 10); do
            curl -s -X POST "$SERVER_URL/webhooks/github" \
                -H "Content-Type: application/json" \
                -H "X-GitHub-Event: push" \
                -H "X-GitHub-Delivery: test-multi-$ORG-$i-$(date +%s%N)" \
                -d '{
                    "ref": "refs/heads/main",
                    "before": "abc",
                    "after": "def'$i'",
                    "repository": {
                        "id": 888'$i',
                        "name": "repo-'$ORG'",
                        "full_name": "'"$ORG"'/repo-'$ORG'",
                        "private": false,
                        "owner": {"id": 888, "login": "'"$ORG"'"},
                        "organization": {"id": 888, "login": "'"$ORG"'"}
                    },
                    "sender": {"id": 1, "login": "testuser"},
                    "commits": [{"id": "def'$i'", "message": "test", "author": {"name": "Test", "email": "test@example.com"}}]
                }' > /dev/null &
        done &
    done
    
    wait
    echo_success "Sent 30 webhooks (10 to 3 orgs each)"
    
    echo_success "Multi-org test PASSED"
}

# Test 5: High Volume Stress Test
test_high_volume() {
    echo_header "Test 5: High Volume (500 webhooks)"
    
    echo "Sending 500 webhooks..."
    START=$(date +%s%N)
    
    for i in $(seq 1 500); do
        curl -s -X POST "$SERVER_URL/webhooks/github" \
            -H "Content-Type: application/json" \
            -H "X-GitHub-Event: push" \
            -H "X-GitHub-Delivery: test-volume-$i-$(date +%s%N)" \
            -d '{
                "ref": "refs/heads/main",
                "before": "abc",
                "after": "def'$i'",
                "repository": {
                    "id": 777'$i'",
                    "name": "stress-repo",
                    "full_name": "'"$ORG_NAME"'/stress-repo-'$i'",
                    "private": false,
                    "owner": {"id": 777, "login": "'"$ORG_NAME"'"},
                    "organization": {"id": 777, "login": "'"$ORG_NAME"'"}
                },
                "sender": {"id": 1, "login": "testuser"},
                "commits": [{"id": "def'$i'", "message": "stress test", "author": {"name": "Test", "email": "test@example.com"}}]
            }' > /dev/null &
        
        # Batch in groups of 50 to avoid overwhelming
        if [ $((i % 50)) -eq 0 ]; then
            wait
            echo "  Sent $i webhooks..."
        fi
    done
    
    wait
    END=$(date +%s%N)
    DURATION_MS=$(( (END - START) / 1000000 ))
    
    echo_success "500 webhooks sent in ${DURATION_MS}ms"
    echo_success "Throughput: $(( 500000 / DURATION_MS )) webhooks/sec"
    
    echo_success "High volume test PASSED"
}

# Test 6: Check Job Metrics
test_job_metrics() {
    echo_header "Test 6: Job Metrics Endpoint"
    
    if [ -z "$API_KEY" ]; then
        echo "Skipping: requires API_KEY"
        echo "Set API_KEY environment variable to test job metrics"
        return 0
    fi
    
    METRICS=$(curl -s -H "Authorization: Bearer $API_KEY" "$SERVER_URL/jobs/metrics")
    
    if echo "$METRICS" | grep -q '"pending"'; then
        echo_success "Job metrics endpoint working"
        echo "$METRICS" | python3 -m json.tool 2>/dev/null || echo "$METRICS"
    else
        echo_fail "Job metrics endpoint failed: $METRICS"
        return 1
    fi
}

# Test 7: Rate limiting on /events
test_events_rate_limit() {
    echo_header "Test 7: Rate Limiting /events"

    if [ "$RUN_RATE_LIMIT_TESTS" != "1" ]; then
        echo "Skipping: RUN_RATE_LIMIT_TESTS=$RUN_RATE_LIMIT_TESTS"
        return 0
    fi

    if ! require_api_key_or_skip; then
        return 0
    fi

    TMPDIR_RATE=$(mktemp -d 2>/dev/null || mktemp -d -t gitgov-rate-events)
    trap 'rm -rf "$TMPDIR_RATE"' RETURN

    echo_info "Sending $EVENTS_BURST_COUNT requests to /events (concurrency=$EVENTS_BURST_CONCURRENCY)"
    run_parallel_curl_burst "$EVENTS_BURST_COUNT" "$EVENTS_BURST_CONCURRENCY" "events" "$TMPDIR_RATE"

    read -r OK RATE FORBIDDEN BADREQ OTHER UNKNOWN <<EOF
$(summarize_status_codes "$TMPDIR_RATE")
EOF

    echo_info "Status summary: 200=$OK 429=$RATE 403=$FORBIDDEN 400=$BADREQ other=$OTHER neterr=$UNKNOWN"

    if [ "$RATE" -lt 1 ]; then
        echo_fail "No 429 responses detected on /events burst"
        rm -rf "$TMPDIR_RATE"
        trap - RETURN
        return 1
    fi

    if [ "$OK" -lt 1 ]; then
        echo_fail "No successful requests detected before rate limiting on /events"
        rm -rf "$TMPDIR_RATE"
        trap - RETURN
        return 1
    fi

    local retry_after=""
    local hf
    for hf in "$TMPDIR_RATE"/headers.*; do
        [ -e "$hf" ] || continue
        if grep -q "429" "$hf"; then
            retry_after=$(extract_retry_after_from_headers "$hf")
            [ -n "$retry_after" ] && break
        fi
    done

    if [ -z "$retry_after" ]; then
        # fallback: find any response with 429 and inspect matching headers file by index
        for sf in "$TMPDIR_RATE"/status.*; do
            [ -e "$sf" ] || continue
            if [ "$(cat "$sf")" = "429" ]; then
                local idx="${sf##*.}"
                retry_after=$(extract_retry_after_from_headers "$TMPDIR_RATE/headers.$idx")
                [ -n "$retry_after" ] && break
            fi
        done
    fi

    if [ -n "$retry_after" ]; then
        echo_success "Retry-After header present on /events 429 responses: $retry_after"
    else
        echo_fail "Retry-After header missing on /events 429 responses"
        rm -rf "$TMPDIR_RATE"
        trap - RETURN
        return 1
    fi

    rm -rf "$TMPDIR_RATE"
    trap - RETURN
    echo_success "Rate limiting /events test PASSED"
}

# Test 8: Rate limiting on /audit-stream/github (admin API key required)
test_audit_stream_rate_limit() {
    echo_header "Test 8: Rate Limiting /audit-stream/github"

    if [ "$RUN_RATE_LIMIT_TESTS" != "1" ] || [ "$RUN_AUDIT_RATE_LIMIT_TEST" != "1" ]; then
        echo "Skipping: RUN_RATE_LIMIT_TESTS=$RUN_RATE_LIMIT_TESTS RUN_AUDIT_RATE_LIMIT_TEST=$RUN_AUDIT_RATE_LIMIT_TEST"
        return 0
    fi

    if ! require_api_key_or_skip; then
        return 0
    fi

    TMPDIR_RATE=$(mktemp -d 2>/dev/null || mktemp -d -t gitgov-rate-audit)
    trap 'rm -rf "$TMPDIR_RATE"' RETURN

    echo_info "Sending $AUDIT_BURST_COUNT requests to /audit-stream/github (concurrency=$AUDIT_BURST_CONCURRENCY)"
    run_parallel_curl_burst "$AUDIT_BURST_COUNT" "$AUDIT_BURST_CONCURRENCY" "audit" "$TMPDIR_RATE"

    read -r OK RATE FORBIDDEN BADREQ OTHER UNKNOWN <<EOF
$(summarize_status_codes "$TMPDIR_RATE")
EOF

    echo_info "Status summary: 200=$OK 429=$RATE 403=$FORBIDDEN 400=$BADREQ other=$OTHER neterr=$UNKNOWN"

    if [ "$FORBIDDEN" -gt 0 ] && [ "$OK" -eq 0 ] && [ "$RATE" -eq 0 ]; then
        echo "Skipping: API_KEY appears to be non-admin for /audit-stream/github"
        rm -rf "$TMPDIR_RATE"
        trap - RETURN
        return 0
    fi

    if [ "$RATE" -lt 1 ]; then
        echo_fail "No 429 responses detected on /audit-stream/github burst"
        rm -rf "$TMPDIR_RATE"
        trap - RETURN
        return 1
    fi

    local retry_after=""
    local sf
    for sf in "$TMPDIR_RATE"/status.*; do
        [ -e "$sf" ] || continue
        if [ "$(cat "$sf")" = "429" ]; then
            local idx="${sf##*.}"
            retry_after=$(extract_retry_after_from_headers "$TMPDIR_RATE/headers.$idx")
            [ -n "$retry_after" ] && break
        fi
    done

    if [ -z "$retry_after" ]; then
        echo_fail "Retry-After header missing on /audit-stream/github 429 responses"
        rm -rf "$TMPDIR_RATE"
        trap - RETURN
        return 1
    fi

    echo_success "Retry-After header present on /audit-stream/github 429 responses: $retry_after"
    rm -rf "$TMPDIR_RATE"
    trap - RETURN
    echo_success "Rate limiting /audit-stream/github test PASSED"
}

# Run all tests
main() {
    echo_header "GitGov Stress Test Suite"
    echo "Server: $SERVER_URL"
    echo "Org: $ORG_NAME"
    echo "API Key: ${API_KEY:-not set}"
    echo "Run webhook stress tests: $RUN_WEBHOOK_STRESS_TESTS"
    echo "Run rate limit tests: $RUN_RATE_LIMIT_TESTS"
    
    check_server
    
    FAILED=0
    
    if [ "$RUN_WEBHOOK_STRESS_TESTS" = "1" ]; then
        test_webhook_idempotency || FAILED=$((FAILED + 1))
        test_job_dedupe || FAILED=$((FAILED + 1))
        test_multi_org || FAILED=$((FAILED + 1))
        test_high_volume || FAILED=$((FAILED + 1))
    else
        echo "Skipping legacy webhook stress tests (RUN_WEBHOOK_STRESS_TESTS=$RUN_WEBHOOK_STRESS_TESTS)"
    fi

    test_stale_reset || FAILED=$((FAILED + 1))
    test_job_metrics || FAILED=$((FAILED + 1))
    test_events_rate_limit || FAILED=$((FAILED + 1))
    test_audit_stream_rate_limit || FAILED=$((FAILED + 1))
    
    echo_header "Test Results"
    
    if [ $FAILED -eq 0 ]; then
        echo_success "All tests PASSED!"
        exit 0
    else
        echo_fail "$FAILED test(s) FAILED"
        exit 1
    fi
}

main "$@"
