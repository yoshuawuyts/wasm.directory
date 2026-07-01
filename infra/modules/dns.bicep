@description('Public DNS zone name, e.g. "wasm.directory". The registrar must delegate this zone to the name servers reported on the created zone (output nameServers).')
param zoneName string

@description('Static (ingress) IP of the Container Apps managed environment. The apex A record points here.')
param staticIp string

@description('The frontend container app customDomainVerificationId. Published as the "asuid" TXT record so Azure can validate domain ownership before issuing a managed certificate.')
param verificationId string

@description('Record TTL in seconds.')
param ttl int = 3600

resource zone 'Microsoft.Network/dnsZones@2018-05-01' = {
  name: zoneName
  location: 'global'
}

// Apex A record: wasm.directory -> environment static ingress IP.
resource apexA 'Microsoft.Network/dnsZones/A@2018-05-01' = {
  parent: zone
  name: '@'
  properties: {
    TTL: ttl
    ARecords: [
      {
        ipv4Address: staticIp
      }
    ]
  }
}

// Domain-ownership record consumed by Azure Container Apps managed
// certificate validation. Must be "asuid" for the apex domain.
resource asuidTxt 'Microsoft.Network/dnsZones/TXT@2018-05-01' = {
  parent: zone
  name: 'asuid'
  properties: {
    TTL: ttl
    TXTRecords: [
      {
        value: [
          verificationId
        ]
      }
    ]
  }
}

@description('Name servers assigned to the zone. Point the registrar delegation at exactly this set.')
output nameServers array = zone.properties.nameServers
