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

@description('Lower bound on frontend replicas. 1 keeps the site always on; 0 scales to zero when idle, at the cost of a cold start.')
@minValue(0)
@maxValue(10)
param minReplicas int = 1

@description('Upper bound on frontend replicas, and therefore on worst-case frontend compute spend.')
@minValue(1)
@maxValue(10)
param maxReplicas int = 1

@description('In-flight HTTP requests per frontend replica before another is added.')
@minValue(1)
@maxValue(1000)
param concurrentRequests int = 100

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
      // Declared explicitly so scaling is visible and tunable; with no `rules`
      // entry the platform silently applies ~10 concurrent requests. Set higher
      // here because this app serves only static assets. One replica, always
      // on. See Cost in docs/azure-deployment.md.
      scale: {
        minReplicas: minReplicas
        maxReplicas: maxReplicas
        rules: [
          {
            name: 'http-scaling'
            http: {
              metadata: {
                concurrentRequests: string(concurrentRequests)
              }
            }
          }
        ]
      }
    }
  }
}

output fqdn string = frontendApp.properties.configuration.ingress.fqdn
output name string = frontendApp.name

@description('Azure-generated token published as the "asuid" TXT record to prove domain ownership before a managed certificate is issued.')
output customDomainVerificationId string = frontendApp.properties.customDomainVerificationId
