resource catanRg 'Microsoft.Resources/resourceGroups@2021-04-01' = {
  name: 'catan-rg'
  location: 'westus3'
}

resource longshotKv 'Microsoft.KeyVault/vaults@2019-09-01' = {
  name: 'longshot-kv'
  location: 'westus'
  properties: {
    sku: {
      family: 'A'
      name: 'standard'
    }
    tenantId: subscription().tenantId
    accessPolicies: []
  }
}

resource cosmosAccount 'Microsoft.DocumentDB/databaseAccounts@2021-03-15' = {
  name: 'user-cosmos-account'
  location: 'westus'
  kind: 'GlobalDocumentDB'
  properties: {
    databaseAccountOfferType: 'Standard'
    locations: [
      {
        locationName: 'West US'
        failoverPriority: 0
      }
    ]
    consistencyPolicy: {
      defaultConsistencyLevel: 'Session'
    }
    capabilities: [
      {
        name: 'EnableServerless'
      }
    ]
  }
}

var databases = [
  'Users-db',
  'Users-db-test'
]

var collections = [
  'Game-Collection',
  'Profile-Collection',
  'User-Collection'
]

resource cosmosDatabases 'Microsoft.DocumentDB/databaseAccounts/sqlDatabases@2021-03-15' = [for database in databases: {
  name: '${cosmosAccount.name}/${database}'
  properties: {}
}]

resource cosmosContainers 'Microsoft.DocumentDB/databaseAccounts/sqlDatabases/containers@2021-03-15' = [for database in databases: [for collection in collections: {
  name: '${cosmosAccount.name}/${database}/${collection}'
  properties: {
    resource: {
      id: collection
      partitionKey: {
        paths: [
          '/partitionKey'
        ]
        kind: 'Hash'
        version: 2
      }
      indexingPolicy: {
        automatic: true
        indexingMode: 'consistent'
        includedPaths: [
          {
            path: '/*'
          }
        ]
        excludedPaths: [
          {
            path: '/"_etag"/?'
          }
        ]
      }
      conflictResolutionPolicy: {
        mode: 'LastWriterWins'
        conflictResolutionPath: '/_ts'
      }
      geospatialConfig: {
        type: 'Geography'
      }
    }
  }
}]]
