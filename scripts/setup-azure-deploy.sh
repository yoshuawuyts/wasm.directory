#!/usr/bin/env bash
# Configure the GitHub Actions secrets and variables that the `deploy` job in
# .github/workflows/release.yml needs to deploy to Azure.
#
# See docs/azure-deployment.md ("Automated deployment via GitHub Actions") for
# how these values are produced (OIDC app registration, federated credential,
# role assignment) and what each one means.
#
# Values are resolved in this order: explicit environment variable, then the
# azd environment store (.azure/<env>/.env, queried with `azd env get-value`),
# then an interactive prompt (secret values are read without echo). azd
# auto-detection is on by default whenever azd and an environment are
# available; pass -n to skip it, or -e <name> to target a specific azd
# environment. Optional values are skipped when left blank. Secrets/variables
# already set on the repository are left untouched unless you pass -f (force
# overwrite).
#
# Pulled from azd when present: AZURE_ENV_NAME, AZURE_LOCATION,
# AZURE_SUBSCRIPTION_ID, AZURE_TENANT_ID, AZURE_RESOURCE_GROUP,
# CUSTOM_DOMAIN_NAME, POSTGRES_ADMIN_PASSWORD. AZURE_CLIENT_ID (the OIDC app
# registration appId) and GHCR_PULL_TOKEN are not stored by azd, so they still
# come from the environment or a prompt.
#
#   Secrets:    AZURE_CLIENT_ID AZURE_TENANT_ID AZURE_SUBSCRIPTION_ID
#               POSTGRES_ADMIN_PASSWORD  GHCR_PULL_TOKEN (optional)
#   Variables:  AZURE_ENV_NAME AZURE_LOCATION
#               AZURE_RESOURCE_GROUP (optional)  CUSTOM_DOMAIN_NAME (optional)
#
# Prerequisites:
#   - GitHub CLI (`gh`) authenticated:  gh auth login
#   - Azure Developer CLI (`azd`) with a provisioned environment — used for
#     auto-detection; optional (skipped with -n or when unavailable)
#   - Azure CLI (`az`) logged in — only needed with -a
#
# Usage:
#   ./scripts/setup-azure-deploy.sh                  # azd auto-detect, prompt for the rest
#   ./scripts/setup-azure-deploy.sh -e wasm-registry # read from a named azd environment
#   ./scripts/setup-azure-deploy.sh -n               # skip azd; prompt for everything
#   AZURE_ENV_NAME=wasm-registry AZURE_LOCATION=centralus \
#     ./scripts/setup-azure-deploy.sh                # take values from the env
#   ./scripts/setup-azure-deploy.sh -a               # backfill tenant+subscription from `az`
#   ./scripts/setup-azure-deploy.sh -f               # overwrite values already set on the repo
#   ./scripts/setup-azure-deploy.sh -r owner/repo    # target a specific repo
#   REPO=owner/repo ./scripts/setup-azure-deploy.sh
set -euo pipefail

REPO="${REPO:-}"
FROM_AZ=false
FORCE=false
USE_AZD=true
AZD_ENV=""

usage() {
  awk 'NR==1 { next } /^#/ { sub(/^# ?/, ""); print; next } { exit }' "$0"
}

while getopts "r:e:nafh" opt; do
  case $opt in
    r) REPO="$OPTARG" ;;
    e) AZD_ENV="$OPTARG" ;;
    n) USE_AZD=false ;;
    a) FROM_AZ=true ;;
    f) FORCE=true ;;
    h) usage; exit 0 ;;
    *) echo "Usage: $0 [-r owner/repo] [-e azd-env] [-n] [-a] [-f] [-h]" >&2; exit 1 ;;
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

# Resolve the repo root so azd works regardless of the invocation directory.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

die() { echo "error: $*" >&2; exit 1; }

# Print the value of azd environment key $1 on stdout, or nothing when the key
# is unset (azd exits non-zero and writes a diagnostic we discard).
azd_get() { # key
  local out
  if [ -n "$AZD_ENV" ]; then
    out="$(azd -C "$ROOT" env get-value "$1" -e "$AZD_ENV" 2>/dev/null)" || return 0
  else
    out="$(azd -C "$ROOT" env get-value "$1" 2>/dev/null)" || return 0
  fi
  printf '%s' "$out"
}

# Prefill any still-unset value from the azd environment store
# (.azure/<env>/.env). Explicit environment variables win over azd; -n skips
# azd entirely; -e <name> selects a specific environment.
if [ "$USE_AZD" = true ] && command -v azd >/dev/null 2>&1; then
  if [ -n "$AZD_ENV" ]; then
    azd -C "$ROOT" env get-values -e "$AZD_ENV" >/dev/null 2>&1 \
      || die "cannot read azd environment '$AZD_ENV' (does it exist? see 'azd env list')"
    azd_label="$AZD_ENV"
  else
    azd_label="$(azd_get AZURE_ENV_NAME)"
  fi
  if [ -n "$azd_label" ]; then
    filled=""
    for key in AZURE_ENV_NAME AZURE_LOCATION AZURE_SUBSCRIPTION_ID \
               AZURE_TENANT_ID AZURE_RESOURCE_GROUP CUSTOM_DOMAIN_NAME \
               POSTGRES_ADMIN_PASSWORD; do
      [ -n "${!key:-}" ] && continue
      val="$(azd_get "$key")"
      [ -n "$val" ] || continue
      printf -v "$key" '%s' "$val"
      filled="$filled $key"
    done
    if [ -n "$filled" ]; then
      echo "==> From azd env ($azd_label): prefilled$filled"
    else
      echo "==> From azd env ($azd_label): nothing to prefill"
    fi
  else
    echo "==> No azd environment selected; prompting for the values azd could supply."
    echo "    Provision one ('azd env new <name>' then 'azd provision'), or pass -e <env>."
  fi
elif [ "$USE_AZD" = true ] && [ -n "$AZD_ENV" ]; then
  die "-e requires the Azure Developer CLI (azd) to be installed"
fi

# Optionally backfill still-unset tenant + subscription from the current az
# session (azd values, if any, already took precedence above).
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
