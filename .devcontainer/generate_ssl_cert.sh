#!/bin/bash

SSL_KEY_FILE="$HOME/.ssh/catan_ssl_key.pem"
SSL_CERT_FILE="$HOME/.ssh/catan_ssl_cert.pem"
function echo_info() {
    printf "${GREEN}%s${NORMAL}\n" "${*}"
}
#
# check a self signed cert and if it doesn't exist, create one

function find_or_create_ssl_cert() {
    if [[ ! -f "$SSL_KEY_FILE" || ! -f "$SSL_CERT_FILE" ]]; then
        echo_info "creating SSL information in $HOME"
        openssl req -x509 -newkey rsa:4096 -keyout "$SSL_KEY_FILE" -out "$SSL_CERT_FILE" -days 365 \
            -nodes -subj "/C=US/ST=Washington/L=Woodinville/O=github.com/OU=joelong01/CN=catan_rust"

        chmod 600 "$SSL_CERT_FILE"
        chmod 600 "$SSL_KEY_FILE"
    else
        echo_info "Found SSL cert information in $SSL_KEY_FILE and $SSL_CERT_FILE"
    fi
}

find_or_create_ssl_cert
export SSL_KEY_FILE
export SSL_CERT_FILE