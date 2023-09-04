#!/bin/bash

# Function to verify Azure login
verify_azure_login() {
    local logged_in
    logged_in=$(az account show)
    if [[ -z "$logged_in" ]]; then
        echo "Logging into Azure..."
        az login
    else
        echo "Already logged in to Azure."
    fi
}

# Function to get Cosmos Auth Token
get_cosmos_auth_token() {
    verify_azure_login
     echo "getting cosmos auth_token"
    if [[ -z "$COSMOS_ACCOUNT_NAME" ]] || [[ -z "$AZURE_RESOURCE_GROUP" ]]; then
        echo "Ensure COSMOS_ACCOUNT_NAME and AZURE_RESOURCE_GROUP are set."
        exit 1
    fi
    COSMOS_AUTH_TOKEN=$(az cosmosdb keys list --name "$COSMOS_ACCOUNT_NAME" --resource-group "$AZURE_RESOURCE_GROUP" --query 'secondaryMasterKey' -o tsv)
    export COSMOS_AUTH_TOKEN
}

# Function to get Azure Communication String
get_azure_communication_string() {
    verify_azure_login
    echo "getting azure communication manager connection string"
    local comm_service_name
    comm_service_name=$(az communication list --resource-group "$AZURE_RESOURCE_GROUP" --query '[0].name' -o tsv)
    AZURE_COMMUNICATION_CONNECTION_STRING=$(az communication list-key --name "$comm_service_name" --resource-group "$AZURE_RESOURCE_GROUP" --query 'secondaryConnectionString' -o tsv)
    export AZURE_COMMUNICATION_CONNECTION_STRING
}

# Function to get Service Phone Number
get_service_phone_number() {
    verify_azure_login
    echo "getting service phone number"
    SERVICE_PHONE_NUMBER=$(az communication phonenumber list --query '[0].phoneNumber' -o tsv)
    export SERVICE_PHONE_NUMBER
}

#Function to generate a login secret key
get_login_secret_key() {
    echo "generating a login secret key"
    LOGIN_SECRET_KEY=$(openssl rand -hex 32)
    export LOGIN_SECRET_KEY
}

# Main script
while [[ "$#" -gt 0 ]]; do
    case $1 in
    --cosmos-token)
        get_cosmos_auth_token
        shift
        ;;
    --comm-string)
        get_azure_communication_string
        shift
        ;;
    --phone-number)
        get_service_phone_number
        shift
        ;;
    --secret-key)
        get_login_secret_key
        shift
        ;;
    *)
        echo "Unknown parameter passed: $1"
        exit 1
        ;;
    esac
done
