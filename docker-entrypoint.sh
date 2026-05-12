#!/bin/sh
set -e

if [ -n "$COMPONENT_DATABASE_URL" ]; then
    echo "Running database migrations..."
    component admin migrate
fi

exec "$@"
