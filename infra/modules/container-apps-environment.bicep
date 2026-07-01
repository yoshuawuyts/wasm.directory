param name string
param location string
param tags object = {}

@description('Log Analytics workspace customer ID (GUID), not the ARM resource ID.')
param logAnalyticsCustomerId string

@secure()
param logAnalyticsSharedKey string

resource environment 'Microsoft.App/managedEnvironments@2024-03-01' = {
  name: name
  location: location
  tags: tags
  properties: {
    appLogsConfiguration: {
      destination: 'log-analytics'
      logAnalyticsConfiguration: {
        customerId: logAnalyticsCustomerId
        sharedKey: logAnalyticsSharedKey
      }
    }
  }
}

output id string = environment.id
output name string = environment.name
output defaultDomain string = environment.properties.defaultDomain

@description('Static outbound/ingress IP of the environment. Custom-domain apex A records point here.')
output staticIp string = environment.properties.staticIp
