param name string
param location string
param tags object = {}

param containerAppsEnvironmentId string

// Placeholder until the real image is built and pushed.
// `azd deploy` will replace this with the actual image reference.
// Note: API_BASE_URL is baked into the WASM binary at build time (Docker build arg),
// not consumed at runtime. The placeholder image ignores this env var.
param image string = 'mcr.microsoft.com/azuredocs/containerapps-helloworld:latest'

resource frontendApp 'Microsoft.App/containerApps@2024-03-01' = {
  name: name
  location: location
  // azd-service-name tag links this resource to the 'frontend' service in azure.yaml
  tags: union(tags, { 'azd-service-name': 'frontend' })
  properties: {
    environmentId: containerAppsEnvironmentId
    configuration: {
      // External ingress: publicly accessible on the auto-generated *.azurecontainerapps.io URL.
      ingress: {
        external: true
        targetPort: 8080
        transport: 'http'
      }
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
