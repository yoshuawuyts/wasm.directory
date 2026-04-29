#!/usr/bin/env bash
# Shared helpers for docker-compose integration tests.

BACKEND_URL="${BACKEND_URL:-http://localhost:8081}"
FRONTEND_URL="${FRONTEND_URL:-http://localhost:8080}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# How long to wait after a backend restart for the initial sync to finish.
SYNC_WAIT="${SYNC_WAIT:-30}"

PASS=0
FAIL=0

green() { printf '\033[0;32m%s\033[0m\n' "$*"; }
red()   { printf '\033[0;31m%s\033[0m\n' "$*"; }
bold()  { printf '\033[1m%s\033[0m\n' "$*"; }

pass() {
    PASS=$((PASS + 1))
    green "  PASS: $*"
}

fail() {
    FAIL=$((FAIL + 1))
    red "  FAIL: $*"
}

# assert_http_status <label> <expected_status> <url> [extra_curl_flags]
assert_http_status() {
    local label="$1" expected="$2" url="$3" extra="${4:-}"
    local actual
    # shellcheck disable=SC2086
    actual=$(curl -s -o /dev/null -w "%{http_code}" $extra "$url")
    if [[ "$actual" == "$expected" ]]; then
        pass "$label (HTTP $actual)"
    else
        fail "$label — expected HTTP $expected, got $actual ($url)"
    fi
}

# assert_json_field <label> <url> <jq_filter> <expected_value>
assert_json_field() {
    local label="$1" url="$2" filter="$3" expected="$4"
    local actual
    actual=$(curl -s "$url" | python3 -c "import sys,json; d=json.load(sys.stdin); print($filter)" 2>/dev/null)
    if [[ "$actual" == "$expected" ]]; then
        pass "$label"
    else
        fail "$label — expected '$expected', got '$actual' ($url)"
    fi
}

# assert_json_contains <label> <url> <jq_python_expression>
# expression should evaluate to truthy/falsy from parsed JSON
assert_json_contains() {
    local label="$1" url="$2" expr="$3"
    local result
    result=$(curl -s "$url" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('yes' if ($expr) else 'no')
" 2>/dev/null)
    if [[ "$result" == "yes" ]]; then
        pass "$label"
    else
        fail "$label — condition not met at $url"
    fi
}

# assert_body_contains <label> <url> <substring>
assert_body_contains() {
    local label="$1" url="$2" substring="$3"
    local body
    body=$(curl -sL "$url")
    if echo "$body" | grep -qF "$substring"; then
        pass "$label (found '$substring')"
    else
        fail "$label — '$substring' not found in body of $url"
    fi
}

# wait_for_url <url> <max_seconds> — polls until the URL returns 200 or times out
wait_for_url() {
    local url="$1" max="$2"
    local i=0
    while [[ $i -lt $max ]]; do
        if curl -sf "$url" > /dev/null 2>&1; then
            return 0
        fi
        sleep 2
        i=$((i + 2))
    done
    return 1
}

# wait_for_backend <seconds> — fixed sleep to allow backend to sync
wait_for_backend() {
    local secs="$1"
    printf '  Waiting %ss for backend sync...' "$secs"
    sleep "$secs"
    echo ' done.'
}

# rebuild_backend — rebuild and restart only the backend container
rebuild_backend() {
    echo "  Rebuilding backend image..."
    docker compose --project-directory "$REPO_ROOT" build backend 2>&1 | tail -3
    echo "  Restarting backend container..."
    docker compose --project-directory "$REPO_ROOT" up -d backend 2>&1 | tail -3
}

# print_summary — print pass/fail counts and exit non-zero on any failure
print_summary() {
    local name="$1"
    bold ""
    bold "=== $name: $PASS passed, $FAIL failed ==="
    [[ $FAIL -eq 0 ]]
}
