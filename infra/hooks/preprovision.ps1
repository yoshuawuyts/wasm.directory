# Registers Azure resource providers required by infra/main.bicep.
# Invoked by azd as a preprovision hook (see azure.yaml).
$ErrorActionPreference = 'Stop'

$providers = @(
    'Microsoft.App',
    'Microsoft.OperationalInsights',
    'Microsoft.ContainerRegistry',
    'Microsoft.DBforPostgreSQL',
    'Microsoft.Insights'
)

Write-Host "Ensuring required Azure resource providers are registered..."

foreach ($ns in $providers) {
    $state = az provider show --namespace $ns --query registrationState -o tsv 2>$null
    switch ($state) {
        'Registered'  { "  {0,-35} already registered" -f $ns | Write-Host }
        'Registering' { "  {0,-35} registration in progress" -f $ns | Write-Host }
        default {
            "  {0,-35} registering..." -f $ns | Write-Host
            az provider register --namespace $ns --only-show-errors | Out-Null
        }
    }
}

Write-Host "Waiting for all providers to reach 'Registered' state..."
foreach ($ns in $providers) {
    for ($i = 0; $i -lt 240; $i++) {
        $state = az provider show --namespace $ns --query registrationState -o tsv
        if ($state -eq 'Registered') { break }
        Start-Sleep -Seconds 5
    }
    if ($state -ne 'Registered') {
        Write-Error "provider $ns did not reach Registered state (current: $state)"
        exit 1
    }
    "  {0,-35} Registered" -f $ns | Write-Host
}
