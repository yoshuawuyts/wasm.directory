#!/bin/sh
set -e

# Migrations run automatically on startup via SeaORM (Store::open_inner).
# No explicit migration command is needed.

exec "$@"
