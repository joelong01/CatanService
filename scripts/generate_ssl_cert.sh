#!/bin/bash

SSL_KEY_FILE="$HOME/.ssh/catan_ssl_key.pem"
SSL_CERT_FILE="$HOME/.ssh/catan_ssl_cert.pem"

RED=$(tput setaf 1)
GREEN=$(tput setaf 2)
YELLOW=$(tput setaf 3)

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
#
#   the script currently works only for MacOS
if [[ "$(uname -s)" != "Darwin" ]]; then
    echo_error "This script only works for MacOS."
    echo_error "For $(uname -s), create and trust a self signed cert named $SSL_CERT_FILE and $SSL_KEY_FILE"
    exit 2
fi

#
# check a self signed cert and if it doesn't exist, create one
function create() {

    if [[ ! -f "$SSL_KEY_FILE" || ! -f "$SSL_CERT_FILE" ]]; then
        echo_info "creating SSL information in $HOME"
        openssl req -x509 -newkey rsa:4096 -keyout "$SSL_KEY_FILE" -out "$SSL_CERT_FILE" -days 365 \
            -nodes -subj "/C=US/ST=Washington/L=Woodinville/O=github.com/OU=joelong01/CN=catan_rust" \
            >/dev/null 2>&1

        chmod 600 "$SSL_CERT_FILE"
        chmod 600 "$SSL_KEY_FILE"

        # add the certificate to the system keychain
        sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain "$SSL_CERT_FILE"

    else
        echo_info "Found SSL cert information in $SSL_KEY_FILE and $SSL_CERT_FILE"
    fi
}
#
# remove the cert from the system and then delete the files
function delete() {
    cert_hash=$(openssl x509 -noout -fingerprint -sha1 -inform pem -in "$SSL_CERT_FILE" |
        awk -F= '{print $2}' |
        tr -d ':')
    echo_warning "Deleting certificate with hash ${cert_hash}."
    sudo security delete-certificate \
        -Z "$cert_hash" \
        -t "$SSL_CERT_FILE" \
        /Library/Keychains/System.keychain

    rm -f "$SSL_KEY_FILE"
    rm -f "$SSL_CERT_FILE"
}

#
function verify() {

    if [[ ! -f $SSL_CERT_FILE ]]; then
        echo_error "$SSL_CERT_FILE does not exist.  cannot verify trust setting."
        exit 1
    fi

    local temp_file="/tmp/trusted_certs.plist"

    # 1. Export the trust settings to a temporary file
    security trust-settings-export -d "$temp_file" >/dev/null

    # 2. Convert the certificate fingerprint to the format used in the trust settings file
    local cert_hash
    cert_hash=$(openssl x509 -noout -fingerprint -sha1 -inform pem -in "$SSL_CERT_FILE" |
        awk -F= '{print $2}' |
        tr -d ':')

    # 3. Check if the certificate hash is in the trust settings file
    grep -q "$cert_hash" "$temp_file"
    local is_trusted=$?

    # 4. Clean up the temporary file
    rm -f "$temp_file"

    # 5. Determine the result
    if [[ $is_trusted ]]; then
        echo_info "The certificate is trusted."
        return 0
    else
        echo_error "The certificate is NOT trusted."
        return 1
    fi
}

while (("$#")); do
    case "$1" in
    create)
        create
        shift
        ;;
    verify)
        verify
        shift
        ;;
    delete)
        delete
        shift
        ;;
    *)
        echo "Invalid option: $1"
        exit 1
        ;;
    esac
done

export SSL_KEY_FILE
export SSL_CERT_FILE
