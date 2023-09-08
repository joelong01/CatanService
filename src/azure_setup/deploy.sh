#!/bin/bash


# Deploy the bicep template
az deployment sub create --location westus3 --template-file resources.bicep --only-show-errors

# Get the auth-token from Cosmos and save it into KeyVault
COSMOS_AUTH_TOKEN=$(az cosmosdb keys list --name user-cosmos-account --resource-group catan-rg --type connection-strings --query connectionStrings[0].connectionString --output tsv)
az keyvault secret set --vault-name longshot-kv --name COSMOS_AUTH_TOKEN --value "$COSMOS_AUTH_TOKEN" --only-show-errors

# Generate a 128 character long random key and save it into KeyVault
LOGIN_SECRET_KEY=$(openssl rand -base64 96)
az keyvault secret set --vault-name longshot-kv --name LOGIN_SECRET_KEY --value "$LOGIN_SECRET_KEY" --only-show-errors
