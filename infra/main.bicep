targetScope = 'subscription'

@minLength(1)
@maxLength(64)
@description('Name of the environment used to generate unique resource names.')
param environmentName string

@description('Azure region for all resources.')
param location string

@description('PostgreSQL administrator login name.')
param postgresAdminLogin string = 'pgadmin'

@secure()
@description('PostgreSQL administrator password (min 8 chars, must include upper, lower, digit, symbol).')
param postgresAdminPassword string

@description('PostgreSQL database name.')
param postgresDatabaseName string = 'componentregistry'

@description('Optional override for the resource group name. Defaults to "rg-<environmentName>".')
param resourceGroupName string = ''

@description('Backend container image. Defaults to a placeholder; override with a real image from ghcr.io.')
param backendImage string = 'mcr.microsoft.com/azuredocs/containerapps-helloworld:latest'

@description('Frontend container image. Defaults to a placeholder; override with a real image from ghcr.io.')
param frontendImage string = 'mcr.microsoft.com/azuredocs/containerapps-helloworld:latest'

@description('Container registry server (e.g. ghcr.io). Leave empty when using public MCR images.')
param registryServer string = ''

@description('Container registry username (e.g. GitHub username for ghcr.io).')
param registryUsername string = ''

@secure()
@description('Container registry password or PAT (e.g. GitHub PAT with read:packages scope).')
param registryPassword string = ''

var rgName = empty(resourceGroupName) ? 'rg-${environmentName}' : resourceGroupName
var tags = { 'azd-env-name': environmentName }

resource rg 'Microsoft.Resources/resourceGroups@2022-09-01' = {
  name: rgName
  location: location
  tags: tags
}

module resources './resources.bicep' = {
  scope: rg
  name: 'resources'
  params: {
    environmentName: environmentName
    location: location
    tags: tags
    postgresAdminLogin: postgresAdminLogin
    postgresAdminPassword: postgresAdminPassword
    postgresDatabaseName: postgresDatabaseName
    backendImage: backendImage
    frontendImage: frontendImage
    registryServer: registryServer
    registryUsername: registryUsername
    registryPassword: registryPassword
  }
}

output AZURE_RESOURCE_GROUP string = rg.name
output AZURE_LOCATION string = location
output AZURE_CONTAINER_APPS_ENVIRONMENT_ID string = resources.outputs.AZURE_CONTAINER_APPS_ENVIRONMENT_ID
output AZURE_CONTAINER_APPS_ENVIRONMENT_NAME string = resources.outputs.AZURE_CONTAINER_APPS_ENVIRONMENT_NAME
output POSTGRES_SERVER_FQDN string = resources.outputs.POSTGRES_SERVER_FQDN
output SERVICE_FRONTEND_URL string = resources.outputs.SERVICE_FRONTEND_URL
output SERVICE_BACKEND_URL string = resources.outputs.SERVICE_BACKEND_URL
