#!/usr/bin/env bash
# Binds the custom apex domain to the frontend container app and requests an
# Azure-managed TLS certificate. Invoked by azd as a postprovision hook (see
# azure.yaml).
#
# This runs out-of-band from the Bicep deployment on purpose: the managed
# certificate can only be issued once the registrar has delegated the zone to
# Azure's name servers AND the "asuid" TXT + apex A records have propagated
# publicly. Wiring it into Bicep would also create a module dependency cycle
# (the cert needs the TXT record, which needs the frontend's verification id).
#
# The hook is a strict no-op when CUSTOM_DOMAIN_NAME is unset, and it never
# fails the provision: if DNS has not propagated yet it prints guidance and
# exits 0 so `azd up` still succeeds. Re-run `azd provision` after delegation
# to complete the bind.
set -uo pipefail

domain="${CUSTOM_DOMAIN_NAME:-}"
if [[ -z "$domain" ]]; then
  exit 0
fi

rg="${AZURE_RESOURCE_GROUP:-}"
app="${SERVICE_FRONTEND_NAME:-frontend}"
env="${AZURE_CONTAINER_APPS_ENVIRONMENT_NAME:-}"

if [[ -z "$rg" || -z "$env" ]]; then
  echo "postprovision: AZURE_RESOURCE_GROUP / AZURE_CONTAINER_APPS_ENVIRONMENT_NAME not set; skipping custom-domain bind." >&2
  exit 0
fi

echo "postprovision: ensuring custom domain '$domain' is bound to '$app'..."

# Already bound with a certificate? Then there is nothing to do.
binding=$(az containerapp hostname list -g "$rg" -n "$app" \
  --query "[?name=='$domain'] | [0].bindingType" -o tsv 2>/dev/null || true)
if [[ "$binding" == "SniEnabled" || "$binding" == "SslBinding" ]]; then
  echo "postprovision: '$domain' already bound ($binding); nothing to do."
  exit 0
fi

# Best-effort public DNS propagation check for the asuid TXT record. If the
# lookup tool is unavailable we skip the check and let Azure validate.
if command -v nslookup >/dev/null 2>&1; then
  if ! nslookup -type=TXT "asuid.$domain" >/dev/null 2>&1; then
    cat >&2 <<EOF
postprovision: 'asuid.$domain' does not resolve publicly yet.
  Delegate the domain at your registrar to the name servers reported in the
  'DNS_NAME_SERVERS' output, wait for propagation, then re-run 'azd provision'.
  Skipping the bind for now (provision still succeeds).
EOF
    exit 0
  fi
fi

# Add the hostname (idempotent: tolerate "already added") then bind it with a
# managed certificate validated via the asuid TXT record.
az containerapp hostname add -g "$rg" -n "$app" --hostname "$domain" --only-show-errors >/dev/null 2>&1 || true

if az containerapp hostname bind -g "$rg" -n "$app" \
  --hostname "$domain" --environment "$env" \
  --validation-method TXT --only-show-errors; then
  echo "postprovision: bound '$domain' and requested a managed certificate."
  echo "postprovision: certificate issuance can take a few minutes to complete."
else
  cat >&2 <<EOF
postprovision: binding '$domain' failed (DNS may not have propagated yet).
  Verify delegation + records, then re-run 'azd provision'. Not failing the
  provision.
EOF
fi

exit 0
