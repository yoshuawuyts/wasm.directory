param name string
param location string
param tags object = {}

@description('Maximum Log Analytics ingestion in GiB per day. Ingestion stops for the remainder of the day when this limit is exceeded.')
param dailyQuotaGb int = 1

@description('Number of days to retain Log Analytics data.')
param retentionInDays int = 7

resource logAnalytics 'Microsoft.OperationalInsights/workspaces@2023-09-01' = {
  name: name
  location: location
  tags: tags
  properties: {
    sku: {
      name: 'PerGB2018'
    }
    retentionInDays: retentionInDays
    workspaceCapping: {
      dailyQuotaGb: dailyQuotaGb
    }
    features: {
      searchVersion: 1
    }
  }
}

// customerId is the workspace GUID used by Container Apps; not the ARM resource ID.
output customerId string = logAnalytics.properties.customerId
output primarySharedKey string = logAnalytics.listKeys().primarySharedKey
output id string = logAnalytics.id
