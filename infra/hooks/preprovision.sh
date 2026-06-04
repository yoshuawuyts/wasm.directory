#!/usr/bin/env bash
# Registers Azure resource providers required by infra/main.bicep.
# Invoked by azd as a preprovision hook (see azure.yaml).
set -euo pipefail

PROVIDERS=(
  Microsoft.App
  Microsoft.OperationalInsights
  Microsoft.ContainerRegistry
  Microsoft.DBforPostgreSQL
  Microsoft.Insights
)

echo "Ensuring required Azure resource providers are registered..."

for ns in "${PROVIDERS[@]}"; do
  state=$(az provider show --namespace "$ns" --query registrationState -o tsv 2>/dev/null || echo "NotFound")
  case "$state" in
    Registered)
      printf '  %-35s %s\n' "$ns" "already registered"
      ;;
    Registering)
      printf '  %-35s %s\n' "$ns" "registration in progress"
      ;;
    *)
      printf '  %-35s %s\n' "$ns" "registering..."
      az provider register --namespace "$ns" --only-show-errors >/dev/null
      ;;
  esac
done

echo "Waiting for all providers to reach 'Registered' state..."
for ns in "${PROVIDERS[@]}"; do
  for _ in {1..240}; do
    state=$(az provider show --namespace "$ns" --query registrationState -o tsv)
    [[ "$state" == "Registered" ]] && break
    sleep 5
  done
  if [[ "$state" != "Registered" ]]; then
    echo "ERROR: provider $ns did not reach Registered state (current: $state)" >&2
    exit 1
  fi
  printf '  %-35s %s\n' "$ns" "Registered"
done
