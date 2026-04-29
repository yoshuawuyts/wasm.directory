#!/usr/bin/env bash
# Degradation test — stop the backend and verify the frontend returns a
# graceful error response (not a 500 crash or a hung connection).
#
# The frontend is a WASM component; it should handle a failed backend call
# and render an error page rather than propagating a 500.
#
# This test restores the backend when done.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

bold "=== Graceful Degradation Test ==="

# ── Pre-condition: stack must be healthy ───────────────────────────────────
bold ""
bold "Pre-conditions"

if ! curl -sf "$BACKEND_URL/v1/health" > /dev/null 2>&1; then
    fail "Backend is not reachable — is the stack running?"
    print_summary "Degradation"
    exit 1
fi
pass "Backend is healthy before test"

if ! curl -sf "$FRONTEND_URL/health" > /dev/null 2>&1; then
    fail "Frontend is not reachable — is the stack running?"
    print_summary "Degradation"
    exit 1
fi
pass "Frontend is healthy before test"

# ── Stop backend ───────────────────────────────────────────────────────────
cleanup() {
    echo ""
    echo "  Restoring backend..."
    docker compose --project-directory "$REPO_ROOT" up -d backend 2>&1 | tail -3
    echo "  Waiting for backend to become healthy..."
    wait_for_url "$BACKEND_URL/v1/health" 30 || true
    echo "  Backend restored."
}
trap cleanup EXIT

bold ""
bold "Stopping backend..."
docker compose --project-directory "$REPO_ROOT" stop backend 2>&1 | tail -2
pass "Backend stopped"

# Give the frontend a moment (it caches nothing — next request will miss)
sleep 2

# ── Frontend with backend down ─────────────────────────────────────────────
bold ""
bold "Testing frontend behaviour with backend down"

# The frontend /health endpoint is served by the WASM component itself and
# must always respond — it does not call the backend.
assert_http_status "GET /health (WASM-only, no backend needed)" 200 \
    "$FRONTEND_URL/health"

# Pages that call the backend should return a non-5xx response.
# The frontend should render an error page (e.g. 200 with error content,
# or a 503), but must NOT hang or return a raw 500 with no body.
SEARCH_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 \
    "$FRONTEND_URL/search?q=wasi")
if [[ "$SEARCH_STATUS" != "500" && "$SEARCH_STATUS" != "000" ]]; then
    pass "GET /search?q=wasi returns $SEARCH_STATUS (not a hard crash) with backend down"
else
    fail "GET /search?q=wasi returned $SEARCH_STATUS with backend down — frontend is not degrading gracefully"
fi

ALL_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 \
    "$FRONTEND_URL/all")
if [[ "$ALL_STATUS" != "500" && "$ALL_STATUS" != "000" ]]; then
    pass "GET /all returns $ALL_STATUS (not a hard crash) with backend down"
else
    fail "GET /all returned $ALL_STATUS with backend down — frontend is not degrading gracefully"
fi

# Requests must complete within timeout (no hung connections)
START=$(date +%s)
curl -s -o /dev/null --max-time 10 "$FRONTEND_URL/all" || true
END=$(date +%s)
ELAPSED=$((END - START))
if [[ $ELAPSED -lt 10 ]]; then
    pass "Frontend responded within ${ELAPSED}s (no hung connection)"
else
    fail "Frontend took ${ELAPSED}s — possible hung connection with backend down"
fi

print_summary "Degradation"
