#!/bin/bash
PASS=0
FAIL=0
RED=$(tput setaf 1)
GREEN=$(tput setaf 2)
YELLOW=$(tput setaf 3)
NORMAL=$(tput sgr0)

AUTH_SERVER_URI="https://localhost:8080/auth/api/v1"
NO_AUTH_SERVER_URI="https://localhost:8080/api/v1"
declare VERBOSE
VERBOSE=false

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
function echo_if_verbose() {
  if [[ $VERBOSE == true ]]; then
    echo_info "$1"
  fi
}
#
#  parses the input to see if we have verbose or a different URI set
function parse_input() {
  # Initialize defaults

  # Parse command-line arguments
  while getopts ":u:v" opt; do
    case ${opt} in
    u)
      SERVER_URI=$OPTARG
      ;;
    v)
      VERBOSE=true
      ;;
    \?)
      echo "Invalid Option: -$OPTARG" 1>&2
      exit 1
      ;;
    :)
      echo "Invalid Option: -$OPTARG requires an argument" 1>&2
      exit 1
      ;;
    esac
  done
  shift $((OPTIND - 1))
}

## check_response: $1 is the return from curl
#                  $2 is the expected retrun from curl (if we wanted to do negative tests)
#                  $3 is the name of the test
#
# tmp.txt contains the actual return from the curl call, which is always in JSON format for this WebAPI
#
function check_response() {
  curl_status=$1
  curl_expected_status=$2
  test_name=$3

  if [[ "$curl_status" -eq $curl_expected_status ]]; then
    echo_info "PASS: $test_name"
    ((PASS++))
  else
    echo_error "FAIL: $test_name (expected $curl_expected_status got $curl_status)"
    ((FAIL++))
  fi
}

function setup_tests() {
  echo ""
  echo_warning "Start setup_tests"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request POST "$NO_AUTH_SERVER_URI/users/setup")
  check_response "$status" 401 "setup, no test header"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request POST "$NO_AUTH_SERVER_URI/users/setup" -H 'is_test: true')
  check_response "$status" 200 "setup with test header"
}
function list_users() {
  echo ""
  echo_warning "Start list_users"
  
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$AUTH_SERVER_URI/auth/users" -H 'is_test: true')
  check_response "$status" 401 "no Authorization Header" "looking for users. negative test"
}
function register_users() {
  echo ""
  echo_warning "Start register_users"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$NO_AUTH_SERVER_URI/users/register" \
    --header 'is_test: true' \
    --header 'X-Password: 1223very long password!' \
    --header 'Content-Type: application/json' \
    --data-raw '{
    "id": "",
    "userProfile": {
        "email": "testi@example.com",
        "firstName": "Doug ",
        "lastName": "Smith",
        "displayName": "Dougy",
        "pictureUrl": "https://www.facebook.com/photo/?fbid=10152713241860783&set=a.485603425782",
        "foregroundColor": "#000000",
        "backgroundColor": "#FFFFFFF",
        "gamesPlayed": 10,
        "gamesWon": 1,
        "textColor": "#000000"
    }
}')
  id=$(jq .id <tmp.txt)
  check_response "$status" 200 "registering user"
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$NO_AUTH_SERVER_URI/users/register" \
    --header 'is_test: true' \
    --header 'X-Password: 1223very long password!' \
    --header 'Content-Type: application/json' \
    --data-raw '{
    "id": "",
    "userProfile": {
        "email": "testi@example.com",
        "firstName": "Doug ",
        "lastName": "Smith",
        "displayName": "Dougy",
        "pictureUrl": "https://www.facebook.com/photo/?fbid=10152713241860783&set=a.485603425782",
        "foregroundColor": "#000000",
        "backgroundColor": "#FFFFFFF",
        "gamesPlayed": 10,
        "gamesWon": 1,
        "textColor": "#000000"
    }
}')

  check_response "$status" 409 "second registration"
}
function test_login() {
  echo ""
  echo_warning "Start test_login"
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$NO_AUTH_SERVER_URI/users/login" \
    --header 'Content-Type: application/json' \
    --header 'is_test: true' \
    --data-raw '{
    "username":"test@example.com",
    "password": "long_password"
}')

  check_response "$status" 404 "bad password"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$NO_AUTH_SERVER_URI/users/login" \
    --header 'is_test: true' \
    --header 'Content-Type: application/json' \
    --data-raw '{
    "username":"testi@example.com",
    "password": "1223very long password!"
}')

  check_response "$status" 200 "login"
  token=$(jq -r .body <tmp.txt)
}

function test_find_users() {
  echo ""
  echo_warning "Start test_find_users"
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$AUTH_SERVER_URI/users" \
    --header "Authorization: $token" \
    --header 'is_test: true')
  check_response "$status" 200 "authenticated get all users"

  id=$(jq -r '.[0].id' <tmp.txt)
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$AUTH_SERVER_URI/users/$id" -H 'is_test: true')
  check_response "$status" 401 "unauthenticated find one user"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$AUTH_SERVER_URI/users/$id" \
    --header 'is_test: true' \
    --header "Authorization: $token")

  check_response "$status" 200 "authenticated find one user"

}
function test_delete_users() {
  echo ""
  echo_warning "Start test_delete_users"
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$AUTH_SERVER_URI/users/$id" \
    --header 'is_test: true')
  check_response "$status" 401 "unauthenticated delete"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$AUTH_SERVER_URI/users/unique_id1184292312312321" \
    --header 'is_test: true' \
    --header "Authorization: $token")
  check_response "$status" 401 "authenticated delete of somebody else"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$AUTH_SERVER_URI/users/$id" \
    --header 'is_test: true' \
    --header "Authorization: $token")

  check_response "$status" 200 "authenticated delete"
}
function run_tests() {

  setup_tests
  list_users
  register_users
  test_login

  SERVER_URI="$SERVER_URI/auth"

  test_find_users
  test_delete_users
}

function print_results() {
  echo_info "PASS: $PASS"
  if [[ $FAIL -gt 0 ]]; then
    echo_error "FAIL: $FAIL"
  else
    echo_info "FAIL: 0"
  fi
}

function clean_up() {
  rm tmp.txt 2>/dev/null
}

parse_input
run_tests
print_results
clean_up
