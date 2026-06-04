#!/usr/bin/env bash
# Full-restart test — bring the entire stack down and back up, then verify
# that SQLite data on the named volume survives a complete container recreation.
#
# This simulates "I shut my laptop off and ran docker compose up again".
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

bold "=== Full Restart Test (down + up) ==="

# ── Pre-shutdown state ─────────────────────────────────────────────────────
bold ""
bold "Recording state before shutdown"

PACKAGE_COUNT_BEFORE=$(curl -s "$BACKEND_URL/v1/packages?limit=200" \
    | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")

if [[ -z "$PACKAGE_COUNT_BEFORE" || "$PACKAGE_COUNT_BEFORE" -eq 0 ]]; then
    fail "No packages found before shutdown — is the stack running?"
    print_summary "Full Restart"
    exit 1
fi
pass "Package count before shutdown: $PACKAGE_COUNT_BEFORE"

# ── Full down ──────────────────────────────────────────────────────────────
bold ""
bold "Bringing stack fully down (volumes preserved)..."
docker compose --project-directory "$REPO_ROOT" down 2>&1 | tail -5
pass "Stack is down"

# ── Full up ────────────────────────────────────────────────────────────────
bold ""
bold "Bringing stack back up..."
docker compose --project-directory "$REPO_ROOT" up -d 2>&1 | tail -5

echo "  Waiting for backend to become healthy..."
if wait_for_url "$BACKEND_URL/v1/health" 60; then
    pass "Backend is healthy after full restart"
else
    fail "Backend did not become healthy within 60s"
    print_summary "Full Restart"
    exit 1
fi

echo "  Waiting for frontend to become healthy..."
if wait_for_url "$FRONTEND_URL/health" 30; then
    pass "Frontend is healthy after full restart"
else
    fail "Frontend did not become healthy within 30s"
    print_summary "Full Restart"
    exit 1
fi

# ── Verify data survived ───────────────────────────────────────────────────
bold ""
bold "Verifying data survived full restart"

PACKAGE_COUNT_AFTER=$(curl -s "$BACKEND_URL/v1/packages?limit=200" \
    | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")

if [[ "$PACKAGE_COUNT_AFTER" -eq "$PACKAGE_COUNT_BEFORE" ]]; then
    pass "Package count unchanged ($PACKAGE_COUNT_AFTER packages) — volume survived"
else
    fail "Package count changed: was $PACKAGE_COUNT_BEFORE, now $PACKAGE_COUNT_AFTER"
fi

# Backend API still functional
assert_http_status "GET /v1/health after full restart"  200 "$BACKEND_URL/v1/health"
assert_http_status "GET /v1/packages after full restart" 200 "$BACKEND_URL/v1/packages"

# Frontend still renders real data
assert_body_contains "Frontend /all still shows packages after full restart" \
    "$FRONTEND_URL/all" "sample-wasi-http-rust"

# Postgres also came back up
assert_http_status "Postgres port reachable after full restart" 200 \
    "$BACKEND_URL/v1/health"   # indirect — just confirms stack is coherent

POSTGRES_CONTAINER="component-cli-postgres-1"
if docker exec "$POSTGRES_CONTAINER" pg_isready -U wasm -d wasm_registry > /dev/null 2>&1; then
    pass "Postgres is ready after full restart"
else
    fail "Postgres is not ready after full restart"
fi

print_summary "Full Restart"
