#!/bin/bash

# this script sets the COSMOS_AUTH_TOKEN to the secondary master key.  requires admin priveleges to the account
# if you don't have admin priveleges

export COSMOS_AUTH_TOKEN

if [ -z "$AZURE_RESOURCE_GROUP" ] || [ -z "$COSMOS_ACCOUNT_NAME" ]; then
    echo "AZURE_RESOURCE_GROUP or COSMOS_ACCOUNT_NAME not set"
    echo "enter the Cosmosdb authentication token (look for 'Keys' in the Azure Portal): "
    read -r COSMOS_AUTH_TOKEN
    return 0
fi

if [[ -z $(az account show 2>/dev/null) ]]; then
    # Login to Azure
    az login
fi

# Get the connection string for your Azure Cosmos DB account
keys=$(az cosmosdb keys list --name "$COSMOS_ACCOUNT_NAME" --resource-group "$AZURE_RESOURCE_GROUP" 2>/dev/null)
if [[ -z $keys ]]; then
    # couldn't get keys -- permission issue?  prompt for it
    echo "enter the Cosmosdb authentication token (look for 'Keys' in the Azure Portal): "
    read -r COSMOS_AUTH_TOKEN
    return 0
fi
# Extract the primary connection string
COSMOS_AUTH_TOKEN=$(echo "$keys" | jq -r '.secondaryMasterKey')
