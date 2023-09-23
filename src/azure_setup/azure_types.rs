/*
{
  "id": "/subscriptions/8399dd24-9717-4302-92c2-590e987c297b/resourceGroups/catan-rg/providers/Microsoft.DocumentDB/databaseAccounts/user-cosmos-account/sqlDatabases/Users-db",
  "location": null,
  "name": "Users-db",
  "options": null,
  "resource": {
    "_self": "dbs/EnVwAA==/",
    "colls": "colls/",
    "etag": "\"00007401-0000-0700-0000-64fbbaf30000\"",
    "id": "Users-db",
    "rid": "EnVwAA==",
    "ts": 1694218995.0,
    "users": "users/"
  },
  "resourceGroup": "catan-rg",
  "tags": null,
  "type": "Microsoft.DocumentDB/databaseAccounts/sqlDatabases"
}
*/

use serde::Deserialize;

#[derive(Deserialize)]
pub struct CosmosDatabaseInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "resourceGroup")]
    pub resource_group: String,
    #[serde(rename = "type")]
    pub cosmos_type: String
}
