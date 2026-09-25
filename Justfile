# component-registry

# Repository to target for `gh` commands. Override with `REPO=owner/name just <recipe>`.
# Defaults to the `origin` remote of the current git checkout.
repo := env_var_or_default("REPO", `git remote get-url origin 2>/dev/null | sed -E 's#(git@github\.com:|https://github\.com/)##; s#\.git$##' | tr -d '\n'`)

# List recipes
default:
    @just --list

# Trigger a release from a given branch (version auto-increments if omitted)
release branch="main" version="":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -n "{{version}}" ]; then
        echo "Triggering release {{version}} from {{branch}}..."
        gh workflow run "Release" --ref {{branch}} -R {{repo}} -f version="{{version}}"
    else
        echo "Triggering release (auto-increment from latest tag) from {{branch}}..."
        gh workflow run "Release" --ref {{branch}} -R {{repo}}
    fi
    sleep 2
    gh run list --workflow="release.yml" -R {{repo}} --limit 1

# Watch the latest release run
release-watch:
    gh run watch $(gh run list --workflow="release.yml" -R {{repo}} --limit 1 --json databaseId -q '.[0].databaseId') -R {{repo}}

# List recent release runs
release-list:
    gh run list --workflow="release.yml" -R {{repo}} --limit 5

# Provision infrastructure with existing images (optional azd environment name)
[positional-arguments]
provision environment="":
    @command -v python3 >/dev/null 2>&1 || { echo "error: provisioning requires Python 3.11 or newer (python3)." >&2; exit 1; }
    @python3 scripts/provision.py "$1"

# Build locally
build:
    cargo build --release --package component

# Run the full test suite (fmt + clippy + tests + sql check)
test:
    cargo xtask test

# Run the server
serve:
    cargo xtask serve
