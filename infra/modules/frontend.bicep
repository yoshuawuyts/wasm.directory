param name string
param location string
param tags object = {}

param containerAppsEnvironmentId string

// Image is passed from resources.bicep; defaults to a placeholder.
// Set FRONTEND_IMAGE via `azd env set` to use a real ghcr.io image.
// Note: API_BASE_URL is baked into the WASM binary at build time (Docker build arg),
// not consumed at runtime. The placeholder image ignores this env var.
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

resource frontendApp 'Microsoft.App/containerApps@2024-03-01' = {
  name: name
  location: location
  tags: tags
  properties: {
    environmentId: containerAppsEnvironmentId
    configuration: {
      // External ingress: publicly accessible on the auto-generated *.azurecontainerapps.io URL.
      ingress: {
        external: true
        targetPort: 8080
        transport: 'http'
      }
      registries: registries
      secrets: registrySecrets
    }
    template: {
      containers: [
        {
          name: 'frontend'
          image: image
          resources: {
            cpu: json('0.25')
            memory: '0.5Gi'
          }
        }
      ]
      scale: {
        minReplicas: 1
        maxReplicas: 3
      }
    }
  }
}

output fqdn string = frontendApp.properties.configuration.ingress.fqdn
output name string = frontendApp.name
