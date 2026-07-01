param name string
param location string
param tags object = {}

param containerAppsEnvironmentId string

@secure()
@description('Full postgres:// connection string including credentials.')
param databaseUrl string

// Image is passed from resources.bicep; defaults to a placeholder.
// Set BACKEND_IMAGE via `azd env set` to use a real ghcr.io image.
param image string = 'mcr.microsoft.com/azuredocs/containerapps-helloworld:latest'

@description('Container registry server (e.g. ghcr.io). Empty string skips registry config.')
param registryServer string = ''

@description('Container registry username.')
param registryUsername string = ''

@secure()
@description('Container registry password or token.')
param registryPassword string = ''

var useRegistry = !empty(registryServer)

var registrySecrets = useRegistry ? [
  {
    name: 'registry-password'
    value: registryPassword
  }
] : []

var registries = useRegistry ? [
  {
    server: registryServer
    username: registryUsername
    passwordSecretRef: 'registry-password'
  }
] : []

resource backendApp 'Microsoft.App/containerApps@2024-03-01' = {
  name: name
  location: location
  tags: tags
  properties: {
    environmentId: containerAppsEnvironmentId
    configuration: {
      // Internal ingress: only reachable within the Container Apps Environment.
      // allowInsecure lets sibling apps reach the backend over plain HTTP on the
      // ingress port (http://backend). Without it, ACA 308-redirects HTTP to
      // HTTPS; the frontend's Wasm HTTP client does not follow that redirect and
      // hangs. Safe here because the app is internal-only (external: false).
      ingress: {
        external: false
        targetPort: 8081
        transport: 'http'
        allowInsecure: true
      }
      registries: registries
      secrets: union([
        {
          name: 'database-url'
          value: databaseUrl
        }
      ], registrySecrets)
    }
    template: {
      containers: [
        {
          name: 'backend'
          image: image
          resources: {
            cpu: json('0.5')
            memory: '1.0Gi'
          }
          env: [
            {
              name: 'COMPONENT_DATABASE_URL'
              secretRef: 'database-url'
            }
            {
              name: 'COMPONENT_DATABASE_MAX_CONNECTIONS'
              value: '8'
            }
            {
              name: 'COMPONENT_DATABASE_CONNECT_TIMEOUT_SECS'
              value: '10'
            }
          ]
        }
      ]
      scale: {
        minReplicas: 1
        maxReplicas: 3
      }
    }
  }
}

output fqdn string = backendApp.properties.configuration.ingress.fqdn
output name string = backendApp.name
