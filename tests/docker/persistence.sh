#!/usr/bin/env bash
# Persistence test — verify that SQLite data on the named volume survives
# a backend container restart without a full re-sync.
#
# Strategy:
#   1. Record the package count before restart.
#   2. Restart the backend container (same image, same volume).
#   3. Wait for the backend to become ready.
#   4. Assert the package count is the same (data was not lost).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

bold "=== Persistence Test ==="

# ── Pre-restart state ──────────────────────────────────────────────────────
bold ""
bold "Recording state before restart"

PACKAGE_COUNT_BEFORE=$(curl -s "$BACKEND_URL/v1/packages?limit=200" \
    | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")

if [[ -z "$PACKAGE_COUNT_BEFORE" || "$PACKAGE_COUNT_BEFORE" -eq 0 ]]; then
    fail "No packages found before restart — is the stack running?"
    print_summary "Persistence"
    exit 1
fi
pass "Package count before restart: $PACKAGE_COUNT_BEFORE"

# ── Restart backend ────────────────────────────────────────────────────────
bold ""
bold "Restarting backend container..."
docker compose --project-directory "$REPO_ROOT" restart backend
echo "  Waiting for backend to become ready..."

# Poll until the health endpoint responds, up to 30 seconds
READY=false
for i in $(seq 1 15); do
    if curl -sf "$BACKEND_URL/v1/health" > /dev/null 2>&1; then
        READY=true
        break
    fi
    sleep 2
done

if [[ "$READY" != "true" ]]; then
    fail "Backend did not become healthy within 30s after restart"
    print_summary "Persistence"
    exit 1
fi
pass "Backend is healthy after restart"

# ── Post-restart state ─────────────────────────────────────────────────────
bold ""
bold "Verifying data after restart"

PACKAGE_COUNT_AFTER=$(curl -s "$BACKEND_URL/v1/packages?limit=200" \
    | python3 -c "import sys,json; print(len(json.load(sys.stdin)))")

if [[ "$PACKAGE_COUNT_AFTER" -eq "$PACKAGE_COUNT_BEFORE" ]]; then
    pass "Package count unchanged after restart ($PACKAGE_COUNT_AFTER packages)"
else
    fail "Package count changed: was $PACKAGE_COUNT_BEFORE, now $PACKAGE_COUNT_AFTER"
fi

# Health and search still work after restart
assert_http_status "GET /v1/health after restart"       200 "$BACKEND_URL/v1/health"
assert_http_status "GET /v1/search?q=wasi after restart" 200 "$BACKEND_URL/v1/search?q=wasi"

# Frontend still responds (wasmtime was not restarted)
assert_http_status "GET /all after backend restart" 200 "$FRONTEND_URL/all"

print_summary "Persistence"
