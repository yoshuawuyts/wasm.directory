#!/usr/bin/env bash
# New-component test — add a new [[component]] entry to an existing namespace
# (ba.toml), rebuild the backend, and verify the component is indexed.
#
# New component: ba:rust-wasi-hello → ghcr.io/bytecodealliance/sample-wasi-http-rust/rust-wasi-hello
#
# The test cleans up after itself (restores ba.toml and rebuilds).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

BA_TOML="$REPO_ROOT/registry/ba.toml"
BA_TOML_BACKUP="$REPO_ROOT/registry/ba.toml.bak"

NEW_COMPONENT="rust-wasi-hello"
NEW_REPO="sample-wasi-http-rust/rust-wasi-hello"
# URL-encoded repository path for the API lookup
NEW_REPO_ENC="sample-wasi-http-rust%2Frust-wasi-hello"

bold "=== New Component Test ==="

# ── Setup ──────────────────────────────────────────────────────────────────
cleanup() {
    echo ""
    echo "  Cleaning up: restoring ba.toml and rebuilding backend..."
    cp "$BA_TOML_BACKUP" "$BA_TOML"
    rm -f "$BA_TOML_BACKUP"
    rebuild_backend
    echo "  Cleanup complete."
}
trap cleanup EXIT

# Pre-condition: the component must not already be indexed
if curl -s "$BACKEND_URL/v1/packages/ghcr.io/bytecodealliance/$NEW_REPO_ENC" \
    | python3 -c "import sys,json; d=json.load(sys.stdin); exit(0 if d else 1)" \
    2>/dev/null; then
    fail "$NEW_COMPONENT already indexed — clean up first"
    print_summary "New Component"
    exit 1
fi
pass "ba:$NEW_COMPONENT not yet indexed (pre-condition)"

# ── Mutate ba.toml ─────────────────────────────────────────────────────────
bold ""
bold "Adding ba:$NEW_COMPONENT to registry/ba.toml..."

cp "$BA_TOML" "$BA_TOML_BACKUP"

cat >> "$BA_TOML" <<TOML

[[component]]
name = "$NEW_COMPONENT"
repository = "$NEW_REPO"
TOML

pass "Appended [[component]] entry to ba.toml"

# ── Rebuild and restart backend ────────────────────────────────────────────
bold ""
bold "Rebuilding backend with updated ba.toml..."
rebuild_backend
wait_for_backend "$SYNC_WAIT"

# ── Verify indexing ────────────────────────────────────────────────────────
bold ""
bold "Verifying indexing"

# Direct package lookup must succeed
assert_http_status "GET ba:$NEW_COMPONENT via /v1/packages/..." 200 \
    "$BACKEND_URL/v1/packages/ghcr.io/bytecodealliance/$NEW_REPO_ENC"

# Must appear in the full package list under the ba namespace
assert_json_contains "ba:$NEW_COMPONENT in /v1/packages" \
    "$BACKEND_URL/v1/packages?limit=100" \
    "any(p.get('wit_namespace') == 'ba' and p.get('wit_name') == '$NEW_COMPONENT' for p in d)"

# Must have at least one tag
assert_json_contains "ba:$NEW_COMPONENT has tags" \
    "$BACKEND_URL/v1/packages/ghcr.io/bytecodealliance/$NEW_REPO_ENC" \
    "len(d.get('tags', [])) > 0"

# Must appear in search by name
assert_json_contains "search '$NEW_COMPONENT' returns results" \
    "$BACKEND_URL/v1/search?q=$NEW_COMPONENT" \
    "isinstance(d, list) and len(d) > 0"

# ── Verify frontend renders the package page ───────────────────────────────
bold ""
bold "Verifying frontend"

# The package namespace page should still render
assert_http_status "GET /ba (namespace page still works)" 200 "$FRONTEND_URL/ba"

print_summary "New Component"
