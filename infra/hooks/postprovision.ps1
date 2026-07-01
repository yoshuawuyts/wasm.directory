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
$ErrorActionPreference = 'Continue'

$domain = $env:CUSTOM_DOMAIN_NAME
if ([string]::IsNullOrEmpty($domain)) {
    exit 0
}

$rg = $env:AZURE_RESOURCE_GROUP
$app = if ($env:SERVICE_FRONTEND_NAME) { $env:SERVICE_FRONTEND_NAME } else { 'frontend' }
$acaEnv = $env:AZURE_CONTAINER_APPS_ENVIRONMENT_NAME

if ([string]::IsNullOrEmpty($rg) -or [string]::IsNullOrEmpty($acaEnv)) {
    Write-Warning "postprovision: AZURE_RESOURCE_GROUP / AZURE_CONTAINER_APPS_ENVIRONMENT_NAME not set; skipping custom-domain bind."
    exit 0
}

Write-Host "postprovision: ensuring custom domain '$domain' is bound to '$app'..."

# Already bound with a certificate? Then there is nothing to do.
$binding = az containerapp hostname list -g $rg -n $app --query "[?name=='$domain'] | [0].bindingType" -o tsv 2>$null
if ($binding -eq 'SniEnabled' -or $binding -eq 'SslBinding') {
    Write-Host "postprovision: '$domain' already bound ($binding); nothing to do."
    exit 0
}

# Best-effort public DNS propagation check for the asuid TXT record. If the
# lookup tool is unavailable we skip the check and let Azure validate.
if (Get-Command nslookup -ErrorAction SilentlyContinue) {
    nslookup -type=TXT "asuid.$domain" 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Warning @"
postprovision: 'asuid.$domain' does not resolve publicly yet.
  Delegate the domain at your registrar to the name servers reported in the
  'DNS_NAME_SERVERS' output, wait for propagation, then re-run 'azd provision'.
  Skipping the bind for now (provision still succeeds).
"@
        exit 0
    }
}

# Add the hostname (idempotent: tolerate "already added") then bind it with a
# managed certificate validated via the asuid TXT record.
az containerapp hostname add -g $rg -n $app --hostname $domain --only-show-errors 2>$null | Out-Null

az containerapp hostname bind -g $rg -n $app --hostname $domain --environment $acaEnv --validation-method TXT --only-show-errors
if ($LASTEXITCODE -eq 0) {
    Write-Host "postprovision: bound '$domain' and requested a managed certificate."
    Write-Host "postprovision: certificate issuance can take a few minutes to complete."
} else {
    Write-Warning @"
postprovision: binding '$domain' failed (DNS may not have propagated yet).
  Verify delegation + records, then re-run 'azd provision'. Not failing the
  provision.
"@
}

exit 0
