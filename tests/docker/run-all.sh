#!/usr/bin/env bash
# run-all.sh — run all docker-compose integration tests in sequence.
#
# Usage:
#   ./tests/docker/run-all.sh              # run all tests
#   ./tests/docker/run-all.sh smoke        # run only smoke tests
#   ./tests/docker/run-all.sh --no-mutate  # skip tests that rebuild images
#
# The stack must already be running before invoking this script:
#   docker compose up -d
#
# Environment variables:
#   BACKEND_URL  — default http://localhost:8081
#   FRONTEND_URL — default http://localhost:8080
#   SYNC_WAIT    — seconds to wait after a backend rebuild (default 30)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

green() { printf '\033[0;32m%s\033[0m\n' "$*"; }
red()   { printf '\033[0;31m%s\033[0m\n' "$*"; }
bold()  { printf '\033[1m%s\033[0m\n' "$*"; }

FILTER="${1:-all}"
NO_MUTATE=false
if [[ "$FILTER" == "--no-mutate" ]]; then
    NO_MUTATE=true
    FILTER="all"
fi

# ── Guard: stack must be up ────────────────────────────────────────────────
BACKEND_URL="${BACKEND_URL:-http://localhost:8081}"
if ! curl -sf "$BACKEND_URL/v1/health" > /dev/null 2>&1; then
    red "ERROR: Backend is not reachable at $BACKEND_URL"
    red "       Run 'docker compose up -d' first."
    exit 1
fi

# ── Run tests ──────────────────────────────────────────────────────────────
TOTAL_PASS=0
TOTAL_FAIL=0

run_test() {
    local name="$1" script="$2"

    if [[ "$FILTER" != "all" && "$FILTER" != "$name" ]]; then
        return 0
    fi

    bold ""
    bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    bold "Running: $name"
    bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    set +e
    bash "$script"
    local exit_code=$?
    set -e

    if [[ $exit_code -eq 0 ]]; then
        TOTAL_PASS=$((TOTAL_PASS + 1))
        green "✓ $name PASSED"
    else
        TOTAL_FAIL=$((TOTAL_FAIL + 1))
        red "✗ $name FAILED (exit $exit_code)"
    fi
}

run_test "smoke"       "$SCRIPT_DIR/smoke.sh"
run_test "persistence" "$SCRIPT_DIR/persistence.sh"
run_test "postgres"    "$SCRIPT_DIR/postgres.sh"
run_test "degradation" "$SCRIPT_DIR/degradation.sh"

if [[ "$NO_MUTATE" == "false" ]]; then
    run_test "new-component"  "$SCRIPT_DIR/new-component.sh"
    run_test "new-namespace"  "$SCRIPT_DIR/new-namespace.sh"
    run_test "full-restart"   "$SCRIPT_DIR/full-restart.sh"
else
    bold ""
    bold "Skipping mutating tests (--no-mutate)"
fi

# ── Summary ────────────────────────────────────────────────────────────────
bold ""
bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
bold "TOTAL: $TOTAL_PASS suites passed, $TOTAL_FAIL suites failed"
bold "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

[[ $TOTAL_FAIL -eq 0 ]]
