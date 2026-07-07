#!/usr/bin/env bash
# Bind the custom apex + `api` subdomain hostnames and their managed TLS
# certificates to the frontend and backend Container Apps.
#
# This automates the mechanical half of docs/azure-deployment.md section 7. It
# is invoked by azd as a `postprovision` hook (see azure.yaml) and is also safe
# to run by hand at any time:
#
#   ./scripts/bind-custom-domains.sh
#
# What stays manual: delegating the DNS zone to Azure at your registrar. That
# depends on your domain registrar and only you can do it. This script detects
# whether delegation has propagated (does the `asuid` TXT record resolve
# publicly?) and, when it has not, prints guidance and exits 0 without failing
# the provision. Re-run it (or just `azd provision` again) once delegation is
# live to finish the bind.
#
# Design notes:
#   - No-op when CUSTOM_DOMAIN_NAME is empty (matches the conditional
#     infra/modules/dns.bicep).
#   - Idempotent: skips `hostname add` when the hostname is already present and
#     skips `hostname bind` when a managed certificate is already bound
#     (bindingType == SniEnabled), so re-running is a clean no-op.
#   - Values come from the environment first (azd exposes provisioning outputs
#     to hooks), falling back to `azd env get-value` for manual runs.
#   - Never fails the provision: unmet prerequisites and transient bind errors
#     are reported as warnings and deferred to the next run.
set -euo pipefail

log()  { printf '%s\n' "$*"; }
warn() { printf '%s\n' "$*" >&2; }

# Resolve a provisioning value: environment variable first (present when azd
# runs this as a hook), then the azd environment store (for manual runs).
resolve() { # NAME
  local name="$1" val="${!1:-}"
  if [ -z "$val" ] && command -v azd >/dev/null 2>&1; then
    val="$(azd env get-value "$name" 2>/dev/null || true)"
    case "$val" in ERROR*|'') val="" ;; esac
  fi
  printf '%s' "$val"
}

DOMAIN="$(resolve CUSTOM_DOMAIN_NAME)"
if [ -z "$DOMAIN" ]; then
  log "==> CUSTOM_DOMAIN_NAME is not set; no custom domain to bind. Skipping."
  exit 0
fi

if ! command -v az >/dev/null 2>&1; then
  warn "==> az (Azure CLI) not found; cannot bind custom domains. Skipping."
  exit 0
fi

RG="$(resolve AZURE_RESOURCE_GROUP)"
ENVNAME="$(resolve AZURE_CONTAINER_APPS_ENVIRONMENT_NAME)"
FRONTEND_APP="$(resolve SERVICE_FRONTEND_NAME)"
BACKEND_APP="$(resolve SERVICE_BACKEND_NAME)"
API_DOMAIN="$(resolve CUSTOM_API_DOMAIN_NAME)"
[ -n "$API_DOMAIN" ] || API_DOMAIN="api.$DOMAIN"

for pair in "RG=$RG" "ENVNAME=$ENVNAME" "FRONTEND_APP=$FRONTEND_APP" "BACKEND_APP=$BACKEND_APP"; do
  if [ -z "${pair#*=}" ]; then
    warn "==> Missing ${pair%%=*}; cannot bind custom domains (is the environment provisioned?). Skipping."
    exit 0
  fi
done

log "==> Binding custom domains in resource group '$RG'"
log "    frontend  $FRONTEND_APP -> $DOMAIN"
log "    backend   $BACKEND_APP -> $API_DOMAIN"

# Current binding for $2 on app $1: "SniEnabled", "Disabled", or empty (absent).
binding_state() { # APP DOMAIN
  az containerapp hostname list --name "$1" --resource-group "$RG" \
    --query "[?name=='$2'].bindingType | [0]" -o tsv 2>/dev/null || true
}

# 0 = the TXT record resolves publicly (zone delegated + propagated),
# 1 = it does not, 2 = cannot tell (no dig/nslookup available).
txt_resolves() { # FQDN
  local out
  if command -v dig >/dev/null 2>&1; then
    out="$(dig +short TXT "$1" 2>/dev/null || true)"
    [ -n "$out" ]
  elif command -v nslookup >/dev/null 2>&1; then
    out="$(nslookup -type=TXT "$1" 2>/dev/null | grep -i 'text =' || true)"
    [ -n "$out" ]
  else
    return 2
  fi
}

# Add the hostname (if needed) and bind its managed certificate.
# Returns 0 when bound (or already bound), 1 when deferred/failed.
bind_domain() { # APP DOMAIN VALIDATION_METHOD IS_APEX
  local app="$1" domain="$2" method="$3" is_apex="$4"
  local state
  state="$(binding_state "$app" "$domain")"
  if [ "$state" = "SniEnabled" ]; then
    log "  $domain — already bound (SNI enabled); skipping"
    return 0
  fi

  local deleg=0
  txt_resolves "asuid.$domain" || deleg=$?
  if [ "$deleg" -eq 1 ]; then
    log "  $domain — DNS zone not delegated yet (asuid.$domain TXT does not resolve); deferring"
    return 1
  elif [ "$deleg" -eq 2 ]; then
    warn "  $domain — cannot verify DNS delegation (no dig/nslookup); attempting bind anyway"
  fi

  if [ -z "$state" ]; then
    log "  $domain — adding custom hostname"
    if ! az containerapp hostname add \
        --resource-group "$RG" --name "$app" --hostname "$domain" >/dev/null 2>&1; then
      warn "  $domain — hostname add failed (DNS likely not fully propagated); deferring"
      return 1
    fi
  fi

  log "  $domain — binding managed certificate (validation: $method)"
  local rc=0
  if [ "$is_apex" = "true" ]; then
    # Apex certificates validate over HTTP; let DigiCert reach the app on plain
    # HTTP for the probe, then restore the HTTP->HTTPS redirect afterwards.
    az containerapp ingress update -n "$app" -g "$RG" --allow-insecure true >/dev/null 2>&1 || true
    az containerapp hostname bind \
      --resource-group "$RG" --name "$app" --hostname "$domain" \
      --environment "$ENVNAME" --validation-method "$method" >/dev/null 2>&1 || rc=$?
    az containerapp ingress update -n "$app" -g "$RG" --allow-insecure false >/dev/null 2>&1 || true
  else
    az containerapp hostname bind \
      --resource-group "$RG" --name "$app" --hostname "$domain" \
      --environment "$ENVNAME" --validation-method "$method" >/dev/null 2>&1 || rc=$?
  fi

  if [ "$rc" -ne 0 ]; then
    warn "  $domain — certificate bind did not complete (rc=$rc); will retry on next run"
    return 1
  fi
  log "  $domain — bound"
  return 0
}

verify() { # URL
  command -v curl >/dev/null 2>&1 || return 0
  local code
  code="$(curl -sS -o /dev/null -m 15 -w '%{http_code}' "$1" 2>/dev/null || echo 000)"
  log "  verify GET $1 -> $code"
}

deferred=0

if bind_domain "$FRONTEND_APP" "$DOMAIN" HTTP true; then
  verify "https://$DOMAIN/"
else
  deferred=$((deferred + 1))
fi

if bind_domain "$BACKEND_APP" "$API_DOMAIN" TXT false; then
  verify "https://$API_DOMAIN/v1/health"
else
  deferred=$((deferred + 1))
fi

if [ "$deferred" -gt 0 ]; then
  ns="$(resolve DNS_NAME_SERVERS)"
  cat >&2 <<EOF

==> $deferred custom-domain bind(s) are still pending. This is expected before
    the DNS zone is delegated to Azure. To finish (a one-time manual step):

    1. Point your registrar's NS records for '$DOMAIN' at the Azure name
       servers for the zone:
EOF
  if [ -n "$ns" ]; then
    printf '         %s\n' "$ns" >&2
  else
    printf '         azd env get-value DNS_NAME_SERVERS\n' >&2
  fi
  cat >&2 <<EOF
    2. Wait for propagation:
         dig +short NS $DOMAIN
         dig +short TXT asuid.$DOMAIN
         dig +short TXT asuid.$API_DOMAIN
    3. Re-run this bind (or 'azd provision' again):
         ./scripts/bind-custom-domains.sh

    See docs/azure-deployment.md section 7 for details.
EOF
  log "==> Custom-domain binding deferred; provisioning itself succeeded."
else
  log "==> Custom domains bound: https://$DOMAIN and https://$API_DOMAIN"
fi

exit 0
