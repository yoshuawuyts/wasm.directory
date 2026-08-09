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

@description('Upper bound on frontend replicas. This is the worst-case compute bill, so keep it just high enough to absorb a burst.')
@minValue(1)
@maxValue(10)
param maxReplicas int = 2

@description('Concurrent in-flight HTTP requests each frontend replica absorbs before the platform adds another one.')
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
      // Scaling is declared explicitly rather than left to the platform. With
      // no `rules` entry, Container Apps applies an implicit HTTP rule at ~10
      // concurrent requests per replica — a default nothing in this repo wrote
      // down. Declaring it makes the behaviour legible and tunable per
      // environment; it is not a fix for anything. This app was observed
      // sitting at three replicas for ~60 hours at a concurrency far under that
      // threshold, and that plateau remains unexplained. See the Cost section
      // of docs/azure-deployment.md.
      //
      // Scale to zero while idle to avoid compute costs; the first request
      // after an idle period incurs a cold start.
      //
      // `concurrentRequests` is set well above the backend's threshold because
      // this app only serves pre-built static assets (HTML plus the compiled
      // Wasm bundle), with no database work per request, so a replica absorbs
      // considerably more concurrency before it saturates.
      scale: {
        minReplicas: 0
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
