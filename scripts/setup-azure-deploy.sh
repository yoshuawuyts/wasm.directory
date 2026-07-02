#!/usr/bin/env bash
# Configure the GitHub Actions secrets and variables that the `deploy` job in
# .github/workflows/release.yml needs to deploy to Azure.
#
# See docs/azure-deployment.md ("Automated deployment via GitHub Actions") for
# how these values are produced (OIDC app registration, federated credential,
# role assignment) and what each one means.
#
# Values are read from environment variables when set, otherwise the script
# prompts for them (secret values are read without echo). Optional values are
# skipped when left blank. Secrets/variables already set on the repository are
# left untouched unless you pass -f (force overwrite).
#
#   Secrets:    AZURE_CLIENT_ID AZURE_TENANT_ID AZURE_SUBSCRIPTION_ID
#               POSTGRES_ADMIN_PASSWORD  GHCR_PULL_TOKEN (optional)
#   Variables:  AZURE_ENV_NAME AZURE_LOCATION
#               AZURE_RESOURCE_GROUP (optional)  CUSTOM_DOMAIN_NAME (optional)
#
# Prerequisites:
#   - GitHub CLI (`gh`) authenticated:  gh auth login
#   - Azure CLI (`az`) logged in — only needed with -a
#
# Usage:
#   ./scripts/setup-azure-deploy.sh                 # prompt for everything
#   AZURE_ENV_NAME=wasm-registry AZURE_LOCATION=centralus \
#     ./scripts/setup-azure-deploy.sh               # take values from the env
#   ./scripts/setup-azure-deploy.sh -a              # fill tenant+subscription from `az`
#   ./scripts/setup-azure-deploy.sh -f              # overwrite values already set on the repo
#   ./scripts/setup-azure-deploy.sh -r owner/repo   # target a specific repo
#   REPO=owner/repo ./scripts/setup-azure-deploy.sh
set -euo pipefail

REPO="${REPO:-}"
FROM_AZ=false
FORCE=false

usage() {
  awk 'NR==1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"
}

while getopts "r:afh" opt; do
  case $opt in
    r) REPO="$OPTARG" ;;
    a) FROM_AZ=true ;;
    f) FORCE=true ;;
    h) usage; exit 0 ;;
    *) echo "Usage: $0 [-r owner/repo] [-a] [-f] [-h]" >&2; exit 1 ;;
  esac
done

command -v gh >/dev/null || { echo "error: gh (GitHub CLI) is not installed" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "error: gh is not authenticated; run 'gh auth login'" >&2; exit 1; }

# Resolve the target repository: -r flag / REPO env, else the current checkout.
if [ -z "$REPO" ]; then
  REPO="$(gh repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null || true)"
fi
[ -n "$REPO" ] || { echo "error: could not determine repository; pass -r owner/repo or set REPO" >&2; exit 1; }
echo "==> Target repository: $REPO"

# Optionally fill tenant + subscription from the current az session.
if [ "$FROM_AZ" = true ]; then
  command -v az >/dev/null || { echo "error: -a requires the Azure CLI (az)" >&2; exit 1; }
  : "${AZURE_TENANT_ID:=$(az account show --query tenantId -o tsv)}"
  : "${AZURE_SUBSCRIPTION_ID:=$(az account show --query id -o tsv)}"
  echo "==> From az: tenant $AZURE_TENANT_ID, subscription $AZURE_SUBSCRIPTION_ID"
fi

# Names already configured on the repo — used to skip values that are already
# set, unless -f is given. Empty (everything treated as unset) if a list fails.
EXISTING_SECRETS="$(gh secret list --repo "$REPO" --json name --jq '.[].name' 2>/dev/null || true)"
EXISTING_VARIABLES="$(gh variable list --repo "$REPO" --json name --jq '.[].name' 2>/dev/null || true)"

# --- helpers ---------------------------------------------------------------

die() { echo "error: $*" >&2; exit 1; }

# True when the newline-separated list in $1 contains the exact line $2.
list_has() { printf '%s\n' "$1" | grep -qxF "$2"; }

# Resolve a value from environment variable $1, else prompt when a terminal is
# available. The prompt is written to stderr so stdout carries only the value.
resolve_value() { # name silent prompt
  local name="$1" silent="$2" prompt="$3"
  local val="${!name:-}"
  if [ -z "$val" ] && [ -t 0 ]; then
    if [ "$silent" = true ]; then
      read -rs -p "$prompt: " val || true; echo >&2
    else
      read -r -p "$prompt: " val || true
    fi
  fi
  printf '%s' "$val"
}

process_secret() { # name prompt required
  local name="$1" prompt="$2" required="$3" val
  if [ "$FORCE" != true ] && list_has "$EXISTING_SECRETS" "$name"; then
    echo "    secret   $name — already set (use -f to overwrite)"
    return
  fi
  val="$(resolve_value "$name" true "$prompt")"
  if [ -z "$val" ]; then
    if [ "$required" = true ]; then die "$name is required (set it in the environment or run in a terminal)"; fi
    echo "    secret   $name — skipped (no value)"
    return
  fi
  # Pipe via stdin so the token never appears in the process table.
  printf '%s' "$val" | gh secret set "$name" --repo "$REPO"
  echo "    secret   $name — set"
}

process_variable() { # name prompt required
  local name="$1" prompt="$2" required="$3" val
  if [ "$FORCE" != true ] && list_has "$EXISTING_VARIABLES" "$name"; then
    echo "    variable $name — already set (use -f to overwrite)"
    return
  fi
  val="$(resolve_value "$name" false "$prompt")"
  if [ -z "$val" ]; then
    if [ "$required" = true ]; then die "$name is required (set it in the environment or run in a terminal)"; fi
    echo "    variable $name — skipped (no value)"
    return
  fi
  gh variable set "$name" --repo "$REPO" --body "$val" >/dev/null
  echo "    variable $name — set"
}

# --- configure -------------------------------------------------------------

echo "==> Configuring secrets on $REPO"
process_secret AZURE_CLIENT_ID         "AZURE_CLIENT_ID (app registration appId)"                        true
process_secret AZURE_TENANT_ID         "AZURE_TENANT_ID"                                                 true
process_secret AZURE_SUBSCRIPTION_ID   "AZURE_SUBSCRIPTION_ID"                                           true
process_secret POSTGRES_ADMIN_PASSWORD "POSTGRES_ADMIN_PASSWORD"                                         true
process_secret GHCR_PULL_TOKEN         "GHCR_PULL_TOKEN (read:packages PAT; blank if images are public)" false

echo "==> Configuring variables on $REPO"
process_variable AZURE_ENV_NAME       "AZURE_ENV_NAME (e.g. wasm-registry)" true
process_variable AZURE_LOCATION       "AZURE_LOCATION (e.g. centralus)"     true
process_variable AZURE_RESOURCE_GROUP "AZURE_RESOURCE_GROUP (optional)"     false
process_variable CUSTOM_DOMAIN_NAME   "CUSTOM_DOMAIN_NAME (optional)"       false

echo "==> Done. Trigger a release (\`just release\`) and watch Environments → production."
