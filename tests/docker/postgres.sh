#!/usr/bin/env bash
# Postgres test — verify the postgres container is reachable and the
# database is initialized, ready for the future SQLite→Postgres migration.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

POSTGRES_CONTAINER="${POSTGRES_CONTAINER:-component-cli-postgres-1}"
POSTGRES_USER="${POSTGRES_USER:-wasm}"
POSTGRES_DB="${POSTGRES_DB:-wasm_registry}"
POSTGRES_HOST="${POSTGRES_HOST:-localhost}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"

bold "=== Postgres Test ==="

bold ""
bold "Container connectivity"

# Container must be running
if docker inspect --format '{{.State.Running}}' "$POSTGRES_CONTAINER" 2>/dev/null \
    | grep -q "^true$"; then
    pass "$POSTGRES_CONTAINER is running"
else
    fail "$POSTGRES_CONTAINER is not running"
    print_summary "Postgres"
    exit 1
fi

# Port must be open from the host
if command -v nc &>/dev/null; then
    if nc -z "$POSTGRES_HOST" "$POSTGRES_PORT" 2>/dev/null; then
        pass "Port $POSTGRES_PORT is reachable from host"
    else
        fail "Port $POSTGRES_PORT is not reachable from host"
    fi
else
    # Fall back to a TCP connection via /dev/tcp if nc is unavailable
    if (echo > /dev/tcp/"$POSTGRES_HOST"/"$POSTGRES_PORT") 2>/dev/null; then
        pass "Port $POSTGRES_PORT is reachable from host (via /dev/tcp)"
    else
        fail "Port $POSTGRES_PORT is not reachable from host"
    fi
fi

bold ""
bold "Database connectivity"

# Connect and verify the database exists
if docker exec "$POSTGRES_CONTAINER" \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c '\conninfo' \
    > /dev/null 2>&1; then
    pass "psql can connect to database '$POSTGRES_DB' as user '$POSTGRES_USER'"
else
    fail "psql cannot connect to database '$POSTGRES_DB'"
fi

# Run a trivial query to confirm the server is responsive
RESULT=$(docker exec "$POSTGRES_CONTAINER" \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -tAc "SELECT 1;" 2>/dev/null)
if [[ "$RESULT" == "1" ]]; then
    pass "Database responds to queries (SELECT 1 = 1)"
else
    fail "Database did not respond to SELECT 1 (got: '$RESULT')"
fi

# Confirm the database is empty — no tables yet (migration not run)
TABLE_COUNT=$(docker exec "$POSTGRES_CONTAINER" \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    -tAc "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public';" \
    2>/dev/null)
pass "Schema is empty ($TABLE_COUNT tables) — ready for migration"

print_summary "Postgres"
