@description('Public DNS zone name, e.g. "wasm.directory". The registrar must delegate this zone to the name servers reported on the created zone (output nameServers).')
param zoneName string

@description('Static (ingress) IP of the Container Apps managed environment. The apex A record points here.')
param staticIp string

@description('The frontend container app customDomainVerificationId. Published as the apex "asuid" TXT record so Azure can validate ownership of the apex domain before issuing a managed certificate.')
param verificationId string

@description('Label of the subdomain the meta-registry API is served on, prepended to zoneName (e.g. "api" -> api.wasm.directory).')
param apiSubdomain string = 'api'

@description('The backend container app customDomainVerificationId. Published as the "asuid.<apiSubdomain>" TXT record so Azure can validate ownership of the api subdomain before issuing its managed certificate.')
param apiVerificationId string

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

// api subdomain A record: api.wasm.directory -> environment static ingress IP.
// Both apps share the environment's single ingress IP; ACA routes to the
// backend by the bound custom hostname.
resource apiA 'Microsoft.Network/dnsZones/A@2018-05-01' = {
  parent: zone
  name: apiSubdomain
  properties: {
    TTL: ttl
    ARecords: [
      {
        ipv4Address: staticIp
      }
    ]
  }
}

// Domain-ownership record for the api subdomain. Subdomains validate via
// "asuid.<label>" (TXT), unlike the apex which uses HTTP validation.
resource apiAsuidTxt 'Microsoft.Network/dnsZones/TXT@2018-05-01' = {
  parent: zone
  name: 'asuid.${apiSubdomain}'
  properties: {
    TTL: ttl
    TXTRecords: [
      {
        value: [
          apiVerificationId
        ]
      }
    ]
  }
}

@description('Name servers assigned to the zone. Point the registrar delegation at exactly this set.')
output nameServers array = zone.properties.nameServers
