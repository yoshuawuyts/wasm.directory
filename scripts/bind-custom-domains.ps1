# Bind the custom apex + `api` subdomain hostnames and their managed TLS
# certificates to the frontend and backend Container Apps.
#
# PowerShell twin of scripts/bind-custom-domains.sh. Invoked by azd as the
# Windows `postprovision` hook (see azure.yaml) and safe to run by hand:
#
#   ./scripts/bind-custom-domains.ps1                 # domain from azd env / env
#   ./scripts/bind-custom-domains.ps1 -Domain wasm.directory  # domain explicit
#
# See the shell script's header and docs/azure-deployment.md section 7 for the
# full rationale. In short: this automates the mechanical hostname add + managed
# certificate bind for both the apex (frontend) and `api` subdomain (backend).
# Delegating the DNS zone to Azure at your registrar stays manual; when it has
# not propagated yet the script prints guidance and exits 0 without failing the
# provision. Re-running is idempotent.

param([string]$Domain)

$ErrorActionPreference = 'Stop'
# Do not let native command stderr (az warnings) throw; we check $LASTEXITCODE.
$PSNativeCommandUseErrorActionPreference = $false

# Resolve a provisioning value: environment variable first (present when azd
# runs this as a hook), then the azd environment store (for manual runs).
function Resolve-Value([string]$Name) {
    $val = [Environment]::GetEnvironmentVariable($Name)
    if (-not $val -and (Get-Command azd -ErrorAction SilentlyContinue)) {
        $val = (azd env get-value $Name 2>$null)
        if ($LASTEXITCODE -ne 0 -or $val -like 'ERROR*') { $val = '' }
    }
    return ($val | Out-String).Trim()
}

# Domain resolution order: explicit -Domain argument first, then
# CUSTOM_DOMAIN_NAME from the environment (azd hook) or the azd env store.
if (-not $Domain) { $Domain = Resolve-Value 'CUSTOM_DOMAIN_NAME' }
if (-not $Domain) {
    Write-Host '==> No custom domain to bind (no argument and CUSTOM_DOMAIN_NAME is unset).'
    Write-Host '    Pass one explicitly to bind by hand, e.g.:'
    Write-Host '        pwsh ./scripts/bind-custom-domains.ps1 -Domain wasm.directory'
    Write-Host '    or set it in the azd environment and re-provision:'
    Write-Host '        azd env set CUSTOM_DOMAIN_NAME wasm.directory; azd provision'
    exit 0
}

if (-not (Get-Command az -ErrorAction SilentlyContinue)) {
    Write-Warning '==> az (Azure CLI) not found; cannot bind custom domains. Skipping.'
    exit 0
}

$Rg          = Resolve-Value 'AZURE_RESOURCE_GROUP'
$EnvName     = Resolve-Value 'AZURE_CONTAINER_APPS_ENVIRONMENT_NAME'
$FrontendApp = Resolve-Value 'SERVICE_FRONTEND_NAME'
$BackendApp  = Resolve-Value 'SERVICE_BACKEND_NAME'
$ApiDomain   = Resolve-Value 'CUSTOM_API_DOMAIN_NAME'
if (-not $ApiDomain) { $ApiDomain = "api.$Domain" }

foreach ($req in @(
        @{ Name = 'AZURE_RESOURCE_GROUP';                    Value = $Rg },
        @{ Name = 'AZURE_CONTAINER_APPS_ENVIRONMENT_NAME';   Value = $EnvName },
        @{ Name = 'SERVICE_FRONTEND_NAME';                   Value = $FrontendApp },
        @{ Name = 'SERVICE_BACKEND_NAME';                    Value = $BackendApp })) {
    if (-not $req.Value) {
        Write-Warning "==> Missing $($req.Name); cannot bind custom domains (is the environment provisioned?). Skipping."
        exit 0
    }
}

Write-Host "==> Binding custom domains in resource group '$Rg'"
Write-Host "    frontend  $FrontendApp -> $Domain"
Write-Host "    backend   $BackendApp -> $ApiDomain"

# Current binding for $D on app $App: 'SniEnabled', 'Disabled', or '' (absent).
function Get-BindingState([string]$App, [string]$D) {
    $state = az containerapp hostname list --name $App --resource-group $Rg `
        --query "[?name=='$D'].bindingType | [0]" -o tsv 2>$null
    if ($LASTEXITCODE -ne 0) { return '' }
    return ($state | Out-String).Trim()
}

# 'yes' = the TXT record resolves publicly (zone delegated + propagated),
# 'no' = it does not, 'unknown' = cannot tell (no resolver available).
function Test-TxtResolves([string]$Fqdn) {
    if (Get-Command Resolve-DnsName -ErrorAction SilentlyContinue) {
        $r = Resolve-DnsName -Name $Fqdn -Type TXT -ErrorAction SilentlyContinue
        if ($r) { return 'yes' } else { return 'no' }
    }
    elseif (Get-Command nslookup -ErrorAction SilentlyContinue) {
        $o = nslookup -type=TXT $Fqdn 2>$null | Select-String -Pattern 'text ='
        if ($o) { return 'yes' } else { return 'no' }
    }
    return 'unknown'
}

# Add the hostname (if needed) and bind its managed certificate.
# Returns $true when bound (or already bound), $false when deferred/failed.
function Invoke-BindDomain([string]$App, [string]$D, [string]$Method) {
    $state = Get-BindingState $App $D
    if ($state -eq 'SniEnabled') {
        Write-Host "  $D - already bound (SNI enabled); skipping"
        return $true
    }

    $deleg = Test-TxtResolves "asuid.$D"
    if ($deleg -eq 'no') {
        Write-Host "  $D - DNS zone not delegated yet (asuid.$D TXT does not resolve); deferring"
        return $false
    }
    elseif ($deleg -eq 'unknown') {
        Write-Warning "  $D - cannot verify DNS delegation (no resolver); attempting bind anyway"
    }

    if (-not $state) {
        Write-Host "  $D - adding custom hostname"
        az containerapp hostname add --resource-group $Rg --name $App --hostname $D 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "  $D - hostname add failed (DNS likely not fully propagated); deferring"
            return $false
        }
    }

    Write-Host "  $D - binding managed certificate (validation: $Method)"
    if ($Method -eq 'HTTP') {
        # HTTP (DigiCert) validation must reach the app over plain HTTP. Capture
        # the current allowInsecure setting, open HTTP for the probe, then
        # restore it: the backend intentionally keeps allowInsecure=true so the
        # in-environment frontend can reach http://backend (see
        # infra/modules/backend.bicep), while the frontend keeps it false. Both
        # hostnames use HTTP validation because managed-certificate TXT
        # validation proved unreliable here (certs stuck in 'Pending').
        $prev = az containerapp ingress show -n $App -g $Rg --query allowInsecure -o tsv 2>$null
        if ($prev -ne 'true') { $prev = 'false' }
        az containerapp ingress update -n $App -g $Rg --allow-insecure true 2>$null | Out-Null
        az containerapp hostname bind --resource-group $Rg --name $App --hostname $D `
            --environment $EnvName --validation-method $Method 2>$null | Out-Null
        $rc = $LASTEXITCODE
        az containerapp ingress update -n $App -g $Rg --allow-insecure $prev 2>$null | Out-Null
    }
    else {
        az containerapp hostname bind --resource-group $Rg --name $App --hostname $D `
            --environment $EnvName --validation-method $Method 2>$null | Out-Null
        $rc = $LASTEXITCODE
    }

    if ($rc -ne 0) {
        Write-Warning "  $D - certificate bind did not complete (rc=$rc); will retry on next run"
        return $false
    }
    Write-Host "  $D - bound"
    return $true
}

function Test-Url([string]$Url) {
    try {
        $code = (Invoke-WebRequest -Uri $Url -Method Get -TimeoutSec 15 `
                -SkipHttpErrorCheck -UseBasicParsing).StatusCode
    }
    catch { $code = 0 }
    Write-Host "  verify GET $Url -> $code"
}

$deferred = 0

if (Invoke-BindDomain $FrontendApp $Domain 'HTTP') { Test-Url "https://$Domain/" }
else { $deferred++ }

if (Invoke-BindDomain $BackendApp $ApiDomain 'HTTP') { Test-Url "https://$ApiDomain/v1/health" }
else { $deferred++ }

if ($deferred -gt 0) {
    $ns = Resolve-Value 'DNS_NAME_SERVERS'
    if (-not $ns) { $ns = 'azd env get-value DNS_NAME_SERVERS' }
    $guidance = @"

==> $deferred custom-domain bind(s) are still pending. This is expected before
    the DNS zone is delegated to Azure. To finish (a one-time manual step):

    1. Point your registrar's NS records for '$Domain' at the Azure name
       servers for the zone:
         $ns
    2. Wait for propagation:
         dig +short NS $Domain
         dig +short TXT asuid.$Domain
         dig +short TXT asuid.$ApiDomain
    3. Re-run this bind (or 'azd provision' again):
         pwsh ./scripts/bind-custom-domains.ps1 -Domain $Domain

    See docs/azure-deployment.md section 7 for details.
"@
    Write-Warning $guidance
    Write-Host '==> Custom-domain binding deferred; provisioning itself succeeded.'
}
else {
    Write-Host "==> Custom domains bound: https://$Domain and https://$ApiDomain"
}

exit 0
