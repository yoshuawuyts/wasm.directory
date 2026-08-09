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

@description('Upper bound on backend replicas. This is the worst-case compute bill, so keep it just high enough to absorb a burst.')
@minValue(1)
@maxValue(10)
param maxReplicas int = 2

@description('Concurrent in-flight HTTP requests each backend replica absorbs before the platform adds another one.')
@minValue(1)
@maxValue(1000)
param concurrentRequests int = 25

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
      // Scaling is declared explicitly rather than left to the platform. With
      // no `rules` entry, Container Apps applies an implicit HTTP rule at ~10
      // concurrent requests per replica, which nothing in this repo documents
      // and which left both apps sitting at their replica ceiling.
      //
      // `minReplicas: 1` keeps one warm replica: the backend holds a Postgres
      // connection pool, so scaling to zero would pay a cold start plus pool
      // re-establishment on the first request after every idle period.
      //
      // `concurrentRequests` is deliberately well above the implicit default.
      // This is a low-traffic package registry serving short, mostly-read API
      // calls, and each replica already fronts a pool of
      // COMPONENT_DATABASE_MAX_CONNECTIONS (8) connections — 25 in-flight
      // requests gives that pool roughly 3x headroom while making scale-out a
      // response to a genuine burst rather than to routine traffic.
      scale: {
        minReplicas: 1
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
