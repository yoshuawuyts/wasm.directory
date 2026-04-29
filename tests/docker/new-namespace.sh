#!/usr/bin/env bash
# New-namespace test — add a fresh namespace TOML, rebuild the backend,
# and verify the packages are indexed and reachable via the API and frontend.
#
# New namespace: "ba-docs" → ghcr.io/bytecodealliance docs/adder + docs/calculator
#
# The test cleans up after itself (removes the TOML and restores the backend).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

NAMESPACE_FILE="$REPO_ROOT/registry/ba-docs.toml"

bold "=== New Namespace Test ==="

# ── Setup ──────────────────────────────────────────────────────────────────
cleanup() {
    echo ""
    echo "  Cleaning up: removing $NAMESPACE_FILE and restoring backend..."
    rm -f "$NAMESPACE_FILE"
    rebuild_backend
    echo "  Cleanup complete."
}
trap cleanup EXIT

# Confirm the namespace does not already exist
if curl -s "$BACKEND_URL/v1/packages" \
    | python3 -c "import sys,json; pkgs=json.load(sys.stdin); \
        exit(0 if any(p.get('wit_namespace')=='ba-docs' for p in pkgs) else 1)" \
    2>/dev/null; then
    fail "ba-docs namespace already present before test — clean up first"
    print_summary "New Namespace"
    exit 1
fi
pass "ba-docs namespace not yet indexed (pre-condition)"

# ── Add the new namespace file ─────────────────────────────────────────────
bold ""
bold "Adding registry/ba-docs.toml..."

cat > "$NAMESPACE_FILE" <<'TOML'
[namespace]
name = "ba-docs"
registry = "ghcr.io/bytecodealliance"

[[component]]
name = "adder"
repository = "docs/adder"

[[component]]
name = "calculator"
repository = "docs/calculator"
TOML

pass "Created registry/ba-docs.toml"

# ── Rebuild and restart backend ────────────────────────────────────────────
bold ""
bold "Rebuilding backend with new namespace..."
rebuild_backend
wait_for_backend "$SYNC_WAIT"

# ── Verify indexing ────────────────────────────────────────────────────────
bold ""
bold "Verifying indexing"

# At least one ba-docs package must appear in the full package list
assert_json_contains "ba-docs packages appear in /v1/packages" \
    "$BACKEND_URL/v1/packages?limit=100" \
    "any(p.get('wit_namespace') == 'ba-docs' for p in d)"

# Specific packages must be directly fetchable
assert_http_status "GET ba-docs:adder" 200 \
    "$BACKEND_URL/v1/packages/ghcr.io/bytecodealliance/docs%2Fadder"

assert_http_status "GET ba-docs:calculator" 200 \
    "$BACKEND_URL/v1/packages/ghcr.io/bytecodealliance/docs%2Fcalculator"

# Search by namespace name must surface the new packages
assert_json_contains "search 'ba-docs' returns results" \
    "$BACKEND_URL/v1/search?q=ba-docs" \
    "isinstance(d, list) and len(d) > 0"

# ── Verify frontend renders the namespace page ─────────────────────────────
bold ""
bold "Verifying frontend"

assert_http_status "GET /ba-docs (namespace page)" 200 "$FRONTEND_URL/ba-docs"

print_summary "New Namespace"
