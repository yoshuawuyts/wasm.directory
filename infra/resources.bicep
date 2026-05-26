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

// ── Outputs ──────────────────────────────────────────────────────────────────

output AZURE_CONTAINER_APPS_ENVIRONMENT_ID string = containerAppsEnv.outputs.id
output AZURE_CONTAINER_APPS_ENVIRONMENT_NAME string = containerAppsEnv.outputs.name
output POSTGRES_SERVER_FQDN string = postgresql.outputs.fqdn
output SERVICE_FRONTEND_URL string = 'https://${frontend.outputs.fqdn}'
output SERVICE_BACKEND_URL string = 'https://${backend.outputs.fqdn}'
