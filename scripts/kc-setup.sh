#!/bin/bash

# Exit on any non-zero status.
set -e
# Initialize action flags
CREATE=0 # We initialize to false (0) here and only set it to true (1) when necessary.
VERIFY=0
DELETE=0


function bool_to_string {
    if [ "$1" -eq 1 ]; then
        echo "true"
    else
        echo "false"
    fi
}

function fn_print_input {
    # Define the maximum length for labels to align table columns.
    local max_label_length=20

    echo "Review your inputs:"
    echo "--------------------------------------"
    printf "%-${max_label_length}s | %s\n" "Parameter" "Value"
    echo "--------------------------------------"
    printf "%-${max_label_length}s | %s\n" "Resource Group" "$AZURE_RG"
    printf "%-${max_label_length}s | %s\n" "Location" "$AZURE_LOC"
    printf "%-${max_label_length}s | %s\n" "AKS Name" "$AKS_CLUSTER_NAME"
    printf "%-${max_label_length}s | %s\n" "Helm Release" "$KEYCLOAK_HELM_RELEASE"
    printf "%-${max_label_length}s | %s\n" "Create" "$(bool_to_string $CREATE)"
    printf "%-${max_label_length}s | %s\n" "Verify" "$(bool_to_string $VERIFY)"
    printf "%-${max_label_length}s | %s\n" "Delete" "$(bool_to_string $DELETE)"
    echo "--------------------------------------"

    # Confirmation prompt
    read -p "Do you wish to continue with these inputs? (Y/n): " -n 1 -r
    echo # move to a new line
    if [[ -n $REPLY && ! $REPLY =~ ^[yY]$ ]]; then
        echo "Operation aborted."
        exit 1
    fi

}

function fn_delete {
    # Ask for confirmation
    read -p "Are you sure you want to delete all Azure resources and Helm state? (y/n): " -n 1 -r
    echo # move to a new line
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        # Step 1: Check if Helm release exists and then delete
        if helm list 2>/dev/null | grep -q "$KEYCLOAK_HELM_RELEASE"; then
            if ! helm uninstall "$KEYCLOAK_HELM_RELEASE"; then
                echo "Error while uninstalling helm release. Skipping to next step..."
            fi
        else
            echo "Helm release '$KEYCLOAK_HELM_RELEASE' not found. Skipping..."
        fi

        # Step 2: Check if AKS cluster exists and then delete
        if az aks show --name "$AKS_CLUSTER_NAME" --resource-group "$AZURE_RG" --output none &>/dev/null; then
            az aks delete --name "$AKS_CLUSTER_NAME" --resource-group "$AZURE_RG" --yes --no-wait
        else
            echo "AKS cluster '$AKS_CLUSTER_NAME' not found. Skipping..."
        fi

        # Step 3: Check if Azure Resource Group exists and then delete
        EXISTENCE=$(az group exists --name "$AZURE_RG")

        if [[ "$EXISTENCE" == "true" ]]; then
            az group delete --name "$AZURE_RG" --yes --no-wait
        else
            echo "Resource group '$AZURE_RG' not found. Skipping..."
        fi
    else
        echo "Deletion aborted."
    fi
}

function fn_verify_installation {
    # Verify AKS Cluster
    echo "Verifying AKS cluster..."
    CLUSTER_STATUS=$(az aks show --resource-group "$AZURE_RG" --name "$AKS_CLUSTER_NAME" --query provisioningState -o tsv)
    if [ "$CLUSTER_STATUS" == "Succeeded" ]; then
        echo "AKS Cluster is successfully provisioned."
    else
        echo "Error: AKS Cluster provisioning has not succeeded. Current status: $CLUSTER_STATUS"
        exit 1
    fi

    # Verify Keycloak Installation
    echo "Verifying Keycloak pods..."
    KEYCLOAK_PODS=$(kubectl get pods -l "app.kubernetes.io/name=keycloak,app.kubernetes.io/instance=keycloak" --no-headers | wc -l | tr -d ' ')
    if [ "$KEYCLOAK_PODS" -gt "0" ]; then
        echo "Keycloak pods are running."
    else
        echo "Error: Keycloak pods not found."
        exit 1
    fi

    # Verify Helm Release
    echo "Verifying Helm release..."
    HELM_RELEASES=$(helm list -f "$KEYCLOAK_HELM_RELEASE" | wc -l)
    if [ "$HELM_RELEASES" -gt "1" ]; then
        echo "Helm release for $KEYCLOAK_HELM_RELEASE found."
    else
        echo "Error: Helm release for $KEYCLOAK_HELM_RELEASE not found."
        exit 1
    fi

    echo "All checks passed! Your installation is verified."
}
function fn_create {
    local RG_ID AKS_ID POD_NAME

    # Step 1: Create a resource group
    echo -n "creating resource group: az group create --name $AZURE_RG --location $AZURE_LOC..."
    RG_ID=$(az group create --name "$AZURE_RG" --location "$AZURE_LOC" --output tsv --query id)
    echo "done. RG_ID=$RG_ID"

    # Step 2: Create AKS cluster
    echo -n "creating AKS cluster: az aks create --resource-group $AZURE_RG --name $AKS_CLUSTER_NAME..."
    AKS_ID=$(az aks create --resource-group "$AZURE_RG" --name "$AKS_CLUSTER_NAME" --node-count 1 --enable-addons monitoring --generate-ssh-keys --output tsv --query id)
    echo "done. AKS_ID=$AKS_ID"

    # Step 3: Get AKS credentials to interact with the cluster using kubectl
    echo -n "getting AKS credentials: az aks get-credentials --resource-group $AZURE_RG --name $AKS_CLUSTER_NAME..."
    az aks get-credentials --resource-group "$AZURE_RG" --name "$AKS_CLUSTER_NAME"
    echo "done."

    # Step 4: Add Helm repo for Keycloak
    echo -n "adding Helm repo for Keycloak: helm repo add codecentric https://codecentric.github.io/helm-charts..."
    helm repo add codecentric https://codecentric.github.io/helm-charts
    helm repo update
    echo "done."

    # Step 5: Install Keycloak using Helm
    echo -n "installing Keycloak using Helm..."
    helm install "$KEYCLOAK_HELM_RELEASE" codecentric/keycloak
    echo "done."

    echo -n "fetching Keycloak POD name..."
    POD_NAME=$(kubectl get pods --namespace default -l "app.kubernetes.io/name=keycloak,app.kubernetes.io/instance=keycloak" -o name)
    echo "done. POD_NAME=$POD_NAME"

    echo "Visit http://127.0.0.1:8080 to use your application"
    kubectl --namespace default port-forward "$POD_NAME" 8080

    echo "kubectl get pods"
    kubectl get pods
    kubectl get svc
    # this will launch quickly and fail since the port isn't forwarded yet, but will quickly resolve once the next
    # line runs
    open http://localhost:8080/auth/
    kubectl port-forward svc/keycloak-http 8080:80

}

# Check if no arguments were passed
if [ $# -eq 0 ]; then
    fn_print_help
    exit 1
fi

function fn_print_help {
    echo "Usage: ./script_name.sh [OPTIONS]"
    echo
    echo "Options:"
    echo "--resource-group     Azure Resource Group name"
    echo "--location           Azure Location/Region"
    echo "--aks-name           Name for AKS Cluster"
    echo "--helm-release       Name for the Keycloak Helm release"
    echo "--verify             Verify the installation"
    echo "--delete             Delete the resources"
    echo "--help               Print this help message"
    echo
}

function fn_parse_input {
    while [ "$1" != "" ]; do
        case $1 in
        --resource-group)
            shift
            AZURE_RG="$1"
            ;;
        --location)
            shift
            AZURE_LOC="$1"
            ;;
        --aks-name)
            shift
            AKS_CLUSTER_NAME="$1"
            ;;
        --helm-release)
            shift
            KEYCLOAK_HELM_RELEASE="$1"
            ;;
        --create)
            CREATE=1
            ;;
        --verify)
            VERIFY=1
            ;;
        --delete)
            DELETE=1
            ;;
        --help)
            fn_print_help
            exit
            ;;
        *)
            echo "ERROR: Unknown parameter \"$1\""
            fn_print_help
            exit 1
            ;;
        esac
        shift
    done

    if [ -z "$AZURE_RG" ] || [ -z "$AZURE_LOC" ] || [ -z "$AKS_CLUSTER_NAME" ] || [ -z "$KEYCLOAK_HELM_RELEASE" ]; then
        echo "ERROR: Missing one of the required parameters for installation."
        fn_print_help
        exit 1
    fi
}

# Parse input arguments
fn_parse_input "$@"
fn_print_input
# Check flags and take action
if [ $DELETE -eq 1 ]; then
    fn_delete
fi

if [ $CREATE -eq 1 ]; then
    fn_create
fi

if [ $VERIFY -eq 1 ]; then
    fn_verify_installation
fi
