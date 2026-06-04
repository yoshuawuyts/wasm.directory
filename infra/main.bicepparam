using './main.bicep'

// azd exports its env store to the process before running bicep, so readEnvironmentVariable works.
param environmentName = readEnvironmentVariable('AZURE_ENV_NAME', '')
param location = readEnvironmentVariable('AZURE_LOCATION', 'westus2')
param resourceGroupName = readEnvironmentVariable('AZURE_RESOURCE_GROUP', '')

param postgresAdminLogin = readEnvironmentVariable('POSTGRES_ADMIN_LOGIN', 'pgadmin')
param postgresAdminPassword = readEnvironmentVariable('POSTGRES_ADMIN_PASSWORD', '')
param postgresDatabaseName = readEnvironmentVariable('POSTGRES_DB', 'componentregistry')

param backendImage = readEnvironmentVariable('BACKEND_IMAGE', 'mcr.microsoft.com/azuredocs/containerapps-helloworld:latest')
param frontendImage = readEnvironmentVariable('FRONTEND_IMAGE', 'mcr.microsoft.com/azuredocs/containerapps-helloworld:latest')

param registryServer = readEnvironmentVariable('REGISTRY_SERVER', '')
param registryUsername = readEnvironmentVariable('REGISTRY_USERNAME', '')
param registryPassword = readEnvironmentVariable('REGISTRY_PASSWORD', '')
