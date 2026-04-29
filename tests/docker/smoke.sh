#!/usr/bin/env bash
# Smoke tests — verify all API endpoints and frontend pages are reachable.
# Requires the docker compose stack to already be running.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

bold "=== Smoke Tests ==="

# ── Backend health ─────────────────────────────────────────────────────────
bold ""
bold "Backend API"

assert_http_status "GET /v1/health"          200 "$BACKEND_URL/v1/health"
assert_json_field  "health status=ok"             "$BACKEND_URL/v1/health" "d['status']" "ok"
assert_http_status "GET /v1/packages"        200 "$BACKEND_URL/v1/packages"
assert_http_status "GET /v1/packages/recent" 200 "$BACKEND_URL/v1/packages/recent"
assert_http_status "GET /v1/search?q=wasi"  200 "$BACKEND_URL/v1/search?q=wasi"
assert_http_status "GET /v1/search/by-import?interface=wasi:http" 200 \
    "$BACKEND_URL/v1/search/by-import?interface=wasi%3Ahttp"
assert_http_status "GET /v1/search/by-export?interface=wasi:http" 200 \
    "$BACKEND_URL/v1/search/by-export?interface=wasi%3Ahttp"

# Known package that must always be present (ba.toml is baked in)
assert_http_status "GET known package (ba:sample-wasi-http-rust)" 200 \
    "$BACKEND_URL/v1/packages/ghcr.io/bytecodealliance/sample-wasi-http-rust%2Fsample-wasi-http-rust"

# Packages list must be non-empty after initial sync
assert_json_contains "packages list is non-empty" \
    "$BACKEND_URL/v1/packages" \
    "isinstance(d, list) and len(d) > 0"

# Search for 'wasi' must return results
assert_json_contains "search for 'wasi' returns results" \
    "$BACKEND_URL/v1/search?q=wasi" \
    "isinstance(d, list) and len(d) > 0"

# ── Frontend pages ─────────────────────────────────────────────────────────
bold ""
bold "Frontend pages"

assert_http_status "GET /health"           200 "$FRONTEND_URL/health"
assert_http_status "GET / (home)"          200 "$FRONTEND_URL/"
assert_http_status "GET /all"              200 "$FRONTEND_URL/all"
# /about is a permanent redirect to /docs; follow it and expect 200
assert_http_status "GET /about (follows redirect to /docs)" 200 "$FRONTEND_URL/about" "-L"
assert_http_status "GET /docs"             200 "$FRONTEND_URL/docs"
assert_http_status "GET /downloads"        200 "$FRONTEND_URL/downloads"
assert_http_status "GET /search?q=wasi"    200 "$FRONTEND_URL/search?q=wasi"
assert_http_status "GET /ba (namespace)"   200 "$FRONTEND_URL/ba"
assert_http_status "GET /wasi (namespace)" 200 "$FRONTEND_URL/wasi"

# ── Frontend renders real backend data ─────────────────────────────────────
bold ""
bold "Frontend data rendering"

# The /all page must contain a known package name from the indexed data
assert_body_contains "GET /all contains 'sample-wasi-http-rust'" \
    "$FRONTEND_URL/all" "sample-wasi-http-rust"

# The /ba namespace page must list ba's packages
assert_body_contains "GET /ba contains 'sample-wasi-http-rust'" \
    "$FRONTEND_URL/ba" "sample-wasi-http-rust"

# Search results must contain matching package names
assert_body_contains "GET /search?q=wasi contains 'wasi'" \
    "$FRONTEND_URL/search?q=wasi" "wasi"

# ── Package detail pages ───────────────────────────────────────────────────
bold ""
bold "Package detail pages"

# Fetch the latest tag for ba:sample-wasi-http-rust from the backend
LATEST_TAG=$(curl -s \
    "$BACKEND_URL/v1/packages/ghcr.io/bytecodealliance/sample-wasi-http-rust%2Fsample-wasi-http-rust" \
    | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['tags'][0])" 2>/dev/null)

if [[ -n "$LATEST_TAG" ]]; then
    pass "Resolved latest tag for ba:sample-wasi-http-rust: $LATEST_TAG"
    assert_http_status "GET /ba/sample-wasi-http-rust/$LATEST_TAG (detail)" 200 \
        "$FRONTEND_URL/ba/sample-wasi-http-rust/$LATEST_TAG"
    assert_body_contains "detail page contains package name" \
        "$FRONTEND_URL/ba/sample-wasi-http-rust/$LATEST_TAG" "sample-wasi-http-rust"
    assert_http_status "GET /ba/sample-wasi-http-rust/$LATEST_TAG/dependencies (follows redirect)" 200 \
        "$FRONTEND_URL/ba/sample-wasi-http-rust/$LATEST_TAG/dependencies" "-L"
else
    fail "Could not resolve a tag for ba:sample-wasi-http-rust — skipping detail checks"
fi

print_summary "Smoke"
