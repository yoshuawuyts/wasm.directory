## Development Environment

This is a Rust project that uses `xtask` for task automation.

## Before Pushing Any Changes

Always run the following command before committing or pushing changes:

```bash
cargo xtask test
```

This command runs:
- `cargo fmt` - Ensures code is properly formatted
- `cargo clippy` - Runs lints to catch common mistakes
- `cargo test` - Runs the test suite
- `cargo xtask sql check` - Applies migrations to in-memory SQLite (and to
  Postgres if `COMPONENT_DATABASE_URL` is set)

**Do not push changes if any of these checks fail.** Fix all formatting issues, clippy warnings, and test failures first.

## Code Style

- Follow Rust idioms and best practices
- All public items should have documentation
- Use `#[must_use]` where appropriate
- Prefer `expect()` over `unwrap()` with descriptive messages

## Code Quality

- Extract loops inside conditionals into their own functions
- Limit indentation to 3 levels; extract deeper logic into helper functions
- Split `if`/`else` blocks where each branch exceeds 60 lines into separate functions
- Replace exhaustive `if let..else` (e.g., `if let Some(..) { .. } else { .. }`) with `match` blocks
- Keep one primary struct/enum per file; split large files accordingly (small helpers exempted)

For detailed guidelines, examples, and a review workflow see the `code-quality` skill.

## Database Schema Changes

The schema is defined as Rust SeaORM migrations under
`crates/component-package-manager-migration/src/migrations/`. To change it:

1. Add a new migration module (`mYYYYMMDD_NNNNNN_<description>.rs`) under
   that directory. Implement the `MigrationTrait::up` (and `down`) methods
   using `SchemaManager` and the entities from
   `component_package_manager_migration::entities`.
2. If new tables are introduced, add the matching entity module under
   `crates/component-package-manager-migration/src/entities/` and re-export
   it from `entities/mod.rs`.
3. Register the migration in `Migrator::migrations()` in
   `crates/component-package-manager-migration/src/lib.rs`.
4. Per-backend SQL fragments (e.g. trigger bodies) belong in
   `migrations/triggers.rs`, dispatched on
   `manager.get_database_backend()`.

Run `cargo xtask sql check` (or `cargo xtask test`) to verify that the new
migration applies cleanly. To exercise the Postgres path, set
`COMPONENT_DATABASE_URL=postgres://...` before running, but only for an
ephemeral/test database: `cargo xtask sql check` runs `Migrator::up` against
the target Postgres instance and will create tables in whatever database the
URL points at. Never point it at a shared, persistent, or production database.

## Database Backend Selection

The `component(1)` CLI and the `component-meta-registry` server both pick
a database backend at runtime via the `COMPONENT_DATABASE_URL` env var:

- Default (no var set): SQLite file under the platform data directory.
  Migrations are applied automatically on startup.
- `sqlite://path/to/file.db?mode=rwc`: explicit SQLite file.
- `postgres://user:pass@host:port/db`: PostgreSQL. Migrations are applied
  automatically on startup too; `Manager::open` serializes the migration
  step with a Postgres advisory lock so concurrent replicas are safe.

Optional tuning vars:
- `COMPONENT_DATABASE_MAX_CONNECTIONS` (Postgres pool size, default 8)
- `COMPONENT_DATABASE_CONNECT_TIMEOUT_SECS` (default 10)
