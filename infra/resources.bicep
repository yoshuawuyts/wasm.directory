targetScope = 'resourceGroup'

@description('Name of the environment used to generate unique resource names.')
param environmentName string

@description('Azure region for all resources.')
param location string

@description('Tags applied to all resources.')
param tags object

@description('PostgreSQL administrator login name.')
param postgresAdminLogin string

@secure()
@description('PostgreSQL administrator password.')
param postgresAdminPassword string

@description('PostgreSQL database name.')
param postgresDatabaseName string

@description('Backend container image.')
param backendImage string

@description('Frontend container image.')
param frontendImage string

@description('Container registry server.')
param registryServer string

@description('Container registry username.')
param registryUsername string

@secure()
@description('Container registry password.')
param registryPassword string

@description('Custom apex domain to serve the frontend on, e.g. "wasm.directory". When empty (default) no DNS zone or custom-domain wiring is created and the app is reachable only on its *.azurecontainerapps.io URL.')
param customDomainName string = ''

var resourceToken = toLower(uniqueString(subscription().id, environmentName, location))

// ── Observability ────────────────────────────────────────────────────────────

module logAnalytics './modules/log-analytics.bicep' = {
  name: 'log-analytics'
  params: {
    name: 'law-${environmentName}-${resourceToken}'
    location: location
    tags: tags
  }
}

// ── Container Apps Environment ───────────────────────────────────────────────

module containerAppsEnv './modules/container-apps-environment.bicep' = {
  name: 'container-apps-env'
  params: {
    name: 'cae-${environmentName}'
    location: location
    tags: tags
    logAnalyticsCustomerId: logAnalytics.outputs.customerId
    logAnalyticsSharedKey: logAnalytics.outputs.primarySharedKey
  }
}

// ── PostgreSQL Flexible Server ───────────────────────────────────────────────

module postgresql './modules/postgresql.bicep' = {
  name: 'postgresql'
  params: {
    serverName: 'pg-${environmentName}-${resourceToken}'
    location: location
    tags: tags
    adminLogin: postgresAdminLogin
    adminPassword: postgresAdminPassword
    databaseName: postgresDatabaseName
  }
}

// ── Container Apps ───────────────────────────────────────────────────────────

module backend './modules/backend.bicep' = {
  name: 'backend'
  params: {
    name: 'backend'
    location: location
    tags: tags
    containerAppsEnvironmentId: containerAppsEnv.outputs.id
    image: backendImage
    databaseUrl: 'postgres://${postgresAdminLogin}:${postgresAdminPassword}@${postgresql.outputs.fqdn}:5432/${postgresDatabaseName}?sslmode=require'
    registryServer: registryServer
    registryUsername: registryUsername
    registryPassword: registryPassword
  }
}

module frontend './modules/frontend.bicep' = {
  name: 'frontend'
  params: {
    name: 'frontend'
    location: location
    tags: tags
    containerAppsEnvironmentId: containerAppsEnv.outputs.id
    image: frontendImage
    registryServer: registryServer
    registryUsername: registryUsername
    registryPassword: registryPassword
  }
}

// ── Custom domain (optional) ─────────────────────────────────────────────────
//
// Creates the public DNS zone plus the records Azure Container Apps needs to
// issue managed certificates for both hostnames:
//   * an apex A record at "@" -> the environment's static ingress IP, plus an
//     "asuid" TXT record carrying the frontend's domainVerificationId, and
//   * an "api" A record (api.<domain>) -> the same static ingress IP, plus an
//     "asuid.api" TXT record carrying the backend's domainVerificationId, so
//     the meta-registry API is reachable at https://api.<domain>.
// The actual hostname bindings + managed certificates are applied out-of-band
// (e.g. via `az containerapp hostname bind`) once the registrar delegates the
// zone and DNS has propagated — doing it here would create a module dependency
// cycle (the cert needs the TXT record, which needs the app's verification id).
module dns './modules/dns.bicep' = if (!empty(customDomainName)) {
  name: 'dns'
  params: {
    zoneName: customDomainName
    staticIp: containerAppsEnv.outputs.staticIp
    verificationId: frontend.outputs.customDomainVerificationId
    apiVerificationId: backend.outputs.customDomainVerificationId
  }
}

// ── Outputs ──────────────────────────────────────────────────────────────────

output AZURE_CONTAINER_APPS_ENVIRONMENT_ID string = containerAppsEnv.outputs.id
output AZURE_CONTAINER_APPS_ENVIRONMENT_NAME string = containerAppsEnv.outputs.name
output POSTGRES_SERVER_FQDN string = postgresql.outputs.fqdn
output SERVICE_FRONTEND_URL string = 'https://${frontend.outputs.fqdn}'
output SERVICE_BACKEND_URL string = 'https://${backend.outputs.fqdn}'
output SERVICE_FRONTEND_NAME string = frontend.outputs.name
output SERVICE_BACKEND_NAME string = backend.outputs.name
output CUSTOM_DOMAIN_NAME string = customDomainName
@description('The api subdomain the meta-registry is served on when a custom domain is configured, e.g. "api.wasm.directory". Empty when no custom domain is set.')
output CUSTOM_API_DOMAIN_NAME string = empty(customDomainName) ? '' : 'api.${customDomainName}'
@description('Name servers assigned to the created DNS zone. Delegate the domain at the registrar to exactly this set. Empty when no custom domain is configured.')
#disable-next-line BCP318 // `dns` is deployed iff customDomainName is non-empty, which the ternary guards.
output DNS_NAME_SERVERS array = empty(customDomainName) ? [] : dns.outputs.nameServers
