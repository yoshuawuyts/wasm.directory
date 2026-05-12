param name string
param location string
param tags object = {}

param containerAppsEnvironmentId string

@secure()
@description('Full postgres:// connection string including credentials.')
param databaseUrl string

// Placeholder until the real image is built and pushed.
// `azd deploy` will replace this with the actual image reference.
param image string = 'mcr.microsoft.com/azuredocs/containerapps-helloworld:latest'

resource backendApp 'Microsoft.App/containerApps@2024-03-01' = {
  name: name
  location: location
  // azd-service-name tag links this resource to the 'backend' service in azure.yaml
  tags: union(tags, { 'azd-service-name': 'backend' })
  properties: {
    environmentId: containerAppsEnvironmentId
    configuration: {
      // Internal ingress: only reachable within the Container Apps Environment.
      ingress: {
        external: false
        targetPort: 8081
        transport: 'http'
      }
      secrets: [
        {
          name: 'database-url'
          value: databaseUrl
        }
      ]
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
