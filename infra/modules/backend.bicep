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

@description('Lower bound on backend replicas. 1 keeps the API always on; 0 scales to zero when idle, at the cost of a cold start.')
@minValue(0)
@maxValue(10)
param minReplicas int = 1

@description('Upper bound on backend replicas, and therefore on worst-case backend compute spend.')
@minValue(1)
@maxValue(10)
param maxReplicas int = 1

@description('In-flight HTTP requests per backend replica before another is added.')
@minValue(1)
@maxValue(1000)
param concurrentRequests int = 10

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
      // External ingress: the meta-registry API is publicly reachable so the
      // `component` CLI (and anything else) can hit it directly at
      // https://api.<domain>. Exposing the backend on its own subdomain keeps
      // it independently reachable and diagnosable even if the frontend
      // website is down.
      //
      // allowInsecure stays true so sibling apps in the environment can still
      // reach the backend over plain HTTP on the ingress port (http://backend):
      // the frontend's Wasm HTTP client talks to it that way and does not
      // follow the 308 HTTP->HTTPS redirect ACA would otherwise send. External
      // callers use HTTPS via the managed certificate; the CLI default is an
      // https:// URL.
      ingress: {
        external: true
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
          // Reduced from 0.5 vCPU / 1.0Gi as a cost optimization; 0.25 vCPU /
          // 0.5Gi has not been load-verified. If replicas are OOM-killed or
          // latency regresses, revert this resources block first. Lower
          // COMPONENT_DATABASE_MAX_CONNECTIONS from 8 if memory pressure appears.
          resources: {
            cpu: json('0.25')
            memory: '0.5Gi'
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
      // Declared explicitly so scaling is visible and tunable; with no `rules`
      // entry the platform silently applies ~10 concurrent requests. Kept at
      // that same 10, because 0.25 vCPU saturates well before ten in-flight
      // requests. One replica, always on. See Cost in docs/azure-deployment.md.
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

output fqdn string = backendApp.properties.configuration.ingress.fqdn
output name string = backendApp.name

@description('Azure-generated token published as the "asuid.api" TXT record to prove ownership of the api subdomain before a managed certificate is issued.')
output customDomainVerificationId string = backendApp.properties.customDomainVerificationId
