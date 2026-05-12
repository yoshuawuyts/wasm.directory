using './main.bicep'

// azd exports its env store to the process before running bicep, so readEnvironmentVariable works.
param environmentName = readEnvironmentVariable('AZURE_ENV_NAME', '')
param location = readEnvironmentVariable('AZURE_LOCATION', 'westus2')
param resourceGroupName = readEnvironmentVariable('AZURE_RESOURCE_GROUP', '')

param postgresAdminLogin = readEnvironmentVariable('POSTGRES_ADMIN_LOGIN', 'pgadmin')
param postgresAdminPassword = readEnvironmentVariable('POSTGRES_ADMIN_PASSWORD', '')
param postgresDatabaseName = readEnvironmentVariable('POSTGRES_DB', 'componentregistry')
