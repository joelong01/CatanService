#!/bin/bash
# Global variables
keyCloakHost=""
keyCloakAdminUsername=""
keyCloakAdminPassword=""
keyCloakRealm=""
keyCloakClientId=""
keyCloakTestUserName="configure_kc.testuser@test.com"
keyCloakTestPassword="password123"

TRACE=
#colors for nice output
RED=$(tput setaf 1)
GREEN=$(tput setaf 2)
YELLOW=$(tput setaf 3)

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

# Functions to echo information in red/yellow/green
function echo_error() {
    printf "${RED}%s${NORMAL}\n" "${*}"
}
function echo_warning() {
    printf "${YELLOW}%s${NORMAL}\n" "${*}"
}
function echo_info() {
    printf "${GREEN}%s${NORMAL}\n" "${*}"
}

function trace() {
    if [[ $TRACE == "true" ]]; then
        echo "${*}"
    fi
}

function fn_print_input {
    # Define the maximum length for labels to align table columns.
    local max_label_length=25

    echo "Review your inputs:"
    echo "--------------------------------------"
    printf "%-${max_label_length}s | %s\n" "Parameter" "Value"
    echo "--------------------------------------"
    printf "%-${max_label_length}s | %s\n" "Action: Create" "$(bool_to_string $CREATE)"
    printf "%-${max_label_length}s | %s\n" "Action: Verify" "$(bool_to_string $VERIFY)"
    printf "%-${max_label_length}s | %s\n" "Action: Delete" "$(bool_to_string $DELETE)"
    printf "%-${max_label_length}s | %s\n" "KeyCloak Host" "$keyCloakHost"
    printf "%-${max_label_length}s | %s\n" "Admin Username" "$keyCloakAdminUsername"
    printf "%-${max_label_length}s | %s\n" "Admin Password" "${keyCloakAdminPassword:0:1}*********" # Hides the actual password, showing only the first char
    printf "%-${max_label_length}s | %s\n" "KeyCloak Realm" "$keyCloakRealm"
    printf "%-${max_label_length}s | %s\n" "KeyCloak ClientId" "$keyCloakClientId"
    echo "--------------------------------------"

    # Confirmation prompt
    read -p "Do you wish to continue with these inputs? (Y/n): " -n 1 -r
    echo # move to a new line
    if [[ -n $REPLY && ! $REPLY =~ ^[yY]$ ]]; then
        echo "Operation aborted."
        exit 1
    fi
}

function fn_print_help {
    echo_info "Usage: ./configure_kc.sh [ACTION] [OPTIONS]"
    echo_info "example:"
    echo_info "./configure_kc.sh create delete verify --host $KEYCLOAK_HOST --admin-username $KEYCLOAK_ADMIN_USER_NAME --admin-password $KEYCLOAK_ADMIN_PASSWORD --realm $KEYCLOAK_REALM --client_id $KEYCLOAK_CLIENT_ID"
    echo_info
    echo_info "Actions:"
    echo_info "  create              Create the resources"
    echo_info "  verify              Verify the installation"
    echo_info "  delete              Delete the resources"
    echo_info
    echo_info "Options:"
    echo_info "  --host HOST          Set the KeyCloak Host"
    echo_info "  --admin-username UN  Set the KeyCloak Admin Username"
    echo_info "  --admin-password PW  Set the KeyCloak Admin Password"
    echo_info "  --realm REALM        Set the KeyCloak Realm"
    echo_info "  --client_id ID        Set the KeyCloak ClientId"
    echo_info "  --help               Print this help message"
    echo_info
}

function fn_parse_input {
    while [ "$1" != "" ]; do
        case $1 in
        create)
            CREATE=1
            ;;
        verify)
            VERIFY=1
            ;;
        delete)
            DELETE=1
            ;;
        --host)
            shift
            keyCloakHost="$1"
            ;;
        --admin-username)
            shift
            keyCloakAdminUsername="$1"
            ;;
        --admin-password)
            shift
            keyCloakAdminPassword="$1"
            ;;
        --realm)
            shift
            keyCloakRealm="$1"
            ;;
        --client_id)
            shift
            keyCloakClientId="$1"
            ;;
        --help)
            fn_print_help
            exit
            ;;
        *)
            echo_error "ERROR: Unknown parameter \"$1\""
            fn_print_help
            exit 1
            ;;
        esac
        shift
    done

    if [ -z "$keyCloakHost" ] || [ -z "$keyCloakAdminUsername" ] || [ -z "$keyCloakAdminPassword" ] ||
        [ -z "$keyCloakClientId" ] || [ -z "$keyCloakRealm" ]; then
        echo_error "ERROR: Missing one of the required parameters for installation."
        echo_error "Please provide --host, --admin-username, --admin-password, and --realm parameters."
        fn_print_help
        exit 1
    fi
}

function realm_exists() {
    local realm="$1"
    local realm_check

    realm_check="$(kcadm.sh get realms --fields realm --format=csv --noquotes)" >/dev/null

    if [[ "$realm_check" == *$realm* ]]; then
        echo "true"
    else
        echo "false"
    fi
}
user_in_role() {
    local realm="$1"
    local username="$2"
    local client_id="$3"
    local role_name="$4"

    local user_id client_uuid role_check

    user_id=$(kcadm.sh get users -r "$realm" --fields id --query username="$username" | jq -r ".[0].id") 

    if [ -z "$user_id" ]; then
        echo "false"
        return
    fi

    client_uuid=$(kcadm.sh get clients -r "$realm" --fields id,clientId --query clientId="$client_id" | jq -r ".[0].id") 
    if [ -z "$client_uuid" ]; then
        echo "false"
        return
    fi

    role_check=$(kcadm.sh get users/"$user_id"/role-mappings/clients/"$client_uuid" -r "$realm" --query name="$role_name" --format csv) 

    if [ -n "$role_check" ]; then
        echo "true"
    else
        echo "false"
    fi
}

function client_role_exists() {
    local realm="$1"
    local client_id="$2"
    local role_name_to_check="$3"
    local client_uuid role_check

    client_uuid=$(kcadm.sh get clients -r "$realm" --fields id --query clientId="$client_id" | jq -r ".[0].id") >/dev/null
    if [ -z "$client_uuid" ]; then
        echo "false"
        return
    fi

    role_check=$(kcadm.sh get clients/"$client_uuid"/roles -r "$realm" --query name="$role_name_to_check" | jq -r ".[0].name") >/dev/null

    if [ "$role_check" == "$role_name_to_check" ]; then
        echo "true"
    else
        echo "false"
    fi
}
# is_caller_in_fole $token $client_id $role
function is_caller_in_role() {
    local token="$1"
    local client_id="$2"
    local role="$3"
    local found_role

    # Extract and decode the header
    header=$(echo "$token" | cut -d "." -f1)
    padding=$(echo "$((4 - ${#header} % 4)) % 4" | bc)

    # Extract and decode the payload
    payload=$(echo "$token" | cut -d "." -f2)
    padding=$(echo "$((4 - ${#payload} % 4)) % 4" | bc)
    decoded_payload=$(echo "${payload}$(head -c "$padding" </dev/zero | tr '\0' '=')" | base64 -D)

    found_role=$(echo "$decoded_payload" | jq ".resource_access.\"$client_id\".roles" | jq -r ".[0]")

    if [[ "$found_role" == "$role" ]]; then
        echo "true"
    else
        echo "false"
    fi

}
function user_exists() {
    local realm="$1"
    local username_to_check="$2"
    local user_check

    user_check=$(kcadm.sh get users \
        -r "$realm" \
        --fields username \
        --query username="$username_to_check" |
        jq -r ".[0].username")

    if [ "$user_check" == "$username_to_check" ]; then
        echo "true"
    else
        echo "false"
    fi
}

fn_create() {
    # Login to Keycloak CLI
    echo_info "logging in as admin"
    kcadm.sh config credentials --server "$keyCloakHost/auth" --realm master \
        --user "$keyCloakAdminUsername" --client admin-cli --password "$keyCloakAdminPassword"

    if [ "$(realm_exists "$keyCloakRealm")" != "true" ]; then
        echo_info "creating realm $keyCloakRealm"
        kcadm.sh create realms -s "realm=$keyCloakRealm" -s enabled=true >/dev/null
    else
        echo_info "$keyCloakRealm already exists. reusing..."
    fi

    # Load ./client.json to create a client
    if [ ! -f "./client.json" ]; then
        echo "./client.json does not exist. Exiting..."
        return 1
    fi

    client_id=$(jq -r ".clientId" <client.json)
    client_uuid=$(kcadm.sh get clients -r "$keyCloakRealm" --fields id,clientId | jq -r ".[] | select(.clientId==\"$client_id\").id") >/dev/null
    if [[ -z "$client_uuid" ]]; then
        echo_info "creating client $client_id"
        kcadm.sh create clients -r "$keyCloakRealm" -f ./client.json >/dev/null
        client_uuid=$(kcadm.sh get clients -r "$keyCloakRealm" --fields id,clientId | jq -r ".[] | select(.clientId==\"$client_id\").id") >/dev/null
        if [ -z "$client_uuid" ]; then
            echo "Failed to fetch the client UUID. Exiting..."
            return 1
        fi
    else
        echo_info "client $client_id already exists. reusing..."
    fi

    if [[ "$(client_role_exists "$keyCloakRealm" "$client_id" TestUser)" != true ]]; then
        echo_info "creating role TestUser"
        # Add a TestUser role to the client
        kcadm.sh create clients/"$client_uuid"/roles -r "$keyCloakRealm" -s name=TestUser -s 'description=Test User Role' >/dev/null
        #  Configure the client to map roles into claims

        kcadm.sh create clients/"$client_uuid"/protocol-mappers/models \
            -r "$keyCloakRealm" \
            -s name=role-mapper \
            -s protocol=openid-connect \
            -s protocolMapper=oidc-usermodel-realm-role-mapper \
            -s 'config."id.token.claim"=true' \
            -s 'config."access.token.claim"=true' \
            -s 'config."userinfo.token.claim"=true' \
            -s 'config."claim.name"=roles' \
            -s 'config."jsonType.label"=String' >/dev/null
    else
        echo_info "TestUser role already exists.  reusing..."
    fi

    
    if [[ "$(user_exists "$keyCloakRealm" "$keyCloakTestUserName")" != "true" ]]; then
        # Create the user in the realm

        kcadm.sh create users \
            -r "$keyCloakRealm" \
            -s "username=$keyCloakTestUserName" \
            -s "email=test@user.com" \
            -s "firstName=Test" \
            -s "lastName=User" \
            -s "enabled=true" >/dev/null
    else
        echo_info "$keyCloakTestUserName already exists. reusing..."
    fi
    # Get user ID
    user_id=$(kcadm.sh get users -r "$keyCloakRealm" --fields id --query username=$keyCloakTestUserName | jq -r ".[0].id") >/dev/null

    #  set the user's password
    if [ -n "$user_id" ]; then
        kcadm.sh set-password -r "$keyCloakRealm" --username "$keyCloakTestUserName" --new-password "$keyCloakTestPassword" >/dev/null
    else
        echo "User not found. Cannot set password."
        return 1
    fi

    if [[ $(user_in_role "$keyCloakRealm" "$keyCloakTestUserName" "$client_id" "TestUser") != true ]]; then
        # Grant the user a role associated with the client.
        # Assuming you have already set the role "TestUser" for the client and you want to grant it to this user.
        kcadm.sh add-roles --target-realm "$keyCloakRealm" \
            --uusername "$keyCloakTestUserName" \
            --cclientid "$client_id" \
            --rolename "TestUser" >/dev/null
    else
        echo_info "$keyCloakTestUserName is already in the client role TestUser"
    fi

    # login with the test creds
    kcadm.sh config credentials --server "$keyCloakHost/auth" \
        --realm "$keyCloakRealm" \
        --client "$client_id" \
        --user "$keyCloakTestUserName" \
        --password "$keyCloakTestPassword"

    # get the token from the key cloak config file
    token=$(jq -r ".endpoints.\"$keyCloakHost/auth\".\"$keyCloakRealm\".token" <"$HOME"/.keycloak/kcadm.config)

    # make sure the client role is there
    if [[ $(is_caller_in_role "$token" "$client_id" "TestUser") != true ]]; then
        echo_error "TestUser not found in $keyCloakTestUserName acocount!  This is an error in the script."
    else
        echo_info "Successfully logged in as $keyCloakTestUserName and verified TestUser role"
    fi

}

fn_verify() {
     # login with the test creds
    kcadm.sh config credentials --server "$keyCloakHost/auth" \
        --realm "$keyCloakRealm" \
        --client "$client_id" \
        --user "$keyCloakTestUserName" \
        --password "$keyCloakTestPassword"

    # get the token from the key cloak config file
    token=$(jq -r ".endpoints.\"$keyCloakHost/auth\".\"$keyCloakRealm\".token" <"$HOME"/.keycloak/kcadm.config)

    # make sure the client role is there
    if [[ $(is_caller_in_role "$token" "$client_id" "TestUser") != true ]]; then
        echo_error "TestUser not found in $keyCloakTestUserName acocount!  run delete and then create"
    else
        echo_info "Successfully logged in as $keyCloakTestUserName and verified TestUser role - Configuration is correct"
    fi

}

fn_delete() {
    read -p "Are you sure you want to delete the realm $keyCloakRealm? There is no Undo. (Nny): " -n 1 -r
    echo # move to a new line
    if [[ -n $REPLY && ! $REPLY =~ ^[yY]$ ]]; then
        echo "Operation aborted."
        exit 1
    fi
    echo_warning "Deleting $keyCloakRealm"

    echo "$GREEN"
    kcadm.sh config credentials --server "$keyCloakHost/auth" --realm master \
        --user "$keyCloakAdminUsername" --client admin-cli --password "$keyCloakAdminPassword"

    # Delete test user
    user_id=$(kcadm.sh get users -r "$keyCloakRealm" -q username="$keyCloakTestUserName" | jq -r '.[0].id')
    if [ -n "$user_id" ] && [ "$user_id" != "null" ]; then
        kcadm.sh delete users/"$user_id" -r "$keyCloakRealm"
        echo "Deleted test user: $keyCloakTestUserName"
    else
        echo "Test user not found: $keyCloakTestUserName"
    fi

    # Delete realm
    kcadm.sh delete realms/"$keyCloakRealm"

    echo "$NORMAL"
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
    fn_verify
fi