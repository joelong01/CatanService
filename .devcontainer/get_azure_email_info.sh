#!/bin/bash
COMMUNICATION_SERVICE_NAME="ct-comm-service"
function get_key {
    if [ -z "$AZURE_RESOURCE_GROUP" ]; then
        echo "AZURE_RESOURCE_GROUP not set"
        echo "enter the Azure Resource Group: "
        read -r AZURE_RESOURCE_GROUP
        export AZURE_RESOURCE_GROUP
    fi
    # Get the connection string for the Azure Communication Service
    AZURE_COMMUNICATION_CONNECTION_STRING=$(az communication list-key --resource-group "$AZURE_RESOURCE_GROUP" --name "$COMMUNICATION_SERVICE_NAME" --query "primaryConnectionString" --output tsv)
    export AZURE_COMMUNICATION_CONNECTION_STRING
}

function login_to_azure {
    if [[ -z $(az account show 2>/dev/null) ]]; then
        # Login to Azure
        az login
    fi
}

login_to_azure
get_key