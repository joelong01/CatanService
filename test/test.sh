#!/bin/bash
PASS=0
FAIL=0
RED=$(tput setaf 1)
GREEN=$(tput setaf 2)
YELLOW=$(tput setaf 3)
SERVER_URI="https://localhost:8080/api/v1"
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


#
# compares 2 json docs using jq.  this way we aren't comparing strings so the formatting isn't as important
compare_json_values() {
  local json1="$1"
  local json2="$2"
  local differences
  differences=$(jq --argjson j1 "$json1" --argjson j2 "$json2" -n '($j1 | (paths(scalars) as $p | getpath($p)) as $v1 | $j2 | getpath($p) as $v2 | select($v1 != $v2))')
  # If differences is not empty, there's at least one difference.
  if [[ -z "$differences" ]]; then
    echo "true"
  else
    echo "false"
  fi
}

## check_response: $1 is the return from curl
#                  $2 is the expected retrun from curl (if we wanted to do negative tests)
#                  $3 is the name of the test
#
# tmp.txt contains the actual return from the curl call, which is always in JSON format for this WebAPI
#
check_response() {
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

function run_tests() {

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request POST "$SERVER_URI/setup")
  check_response "$status" 401 "setup, no test header"


  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request POST "$SERVER_URI/setup" -H 'is_test: true')
  check_response "$status" 200 "setup with test header"


  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" -H 'is_test: true')
  check_response "$status" 401 "no Authorization Header" "looking for users. negative test"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" \
    --header 'is_test: true' \
    --header 'Content-Type: application/json' \
    --data-raw '{
  "password_hash": "",
  "password": "1223very long password!",
  "email": "testi@example.com",
  "first_name": "Doug ",
  "last_name": "Smith",
  "display_name": "Dougy",
  "picture_url": "https://www.facebook.com/photo/?fbid=10152713241860783&set=a.485603425782",
  "foreground_color": "#000000",
  "background_color": "#FFFFFFF",
  "games_played": 10,
  "games_won": 1
}')
  id=$(jq .id <tmp.txt)
  check_response "$status" 200 "registering user"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" \
    --header 'is_test: true' \
    --header 'Content-Type: application/json' \
    --data-raw '{
  "password_hash": "",
  "password": "1223very long password!",
  "email": "testi@example.com",
  "first_name": "Doug ",
  "last_name": "Smith",
  "display_name": "Dougy",
  "picture_url": "https://www.facebook.com/photo/?fbid=10152713241860783&set=a.485603425782",
  "foreground_color": "#000000",
  "background_color": "#FFFFFFF",
  "games_played": 10,
  "games_won": 1
}')

  check_response "$status" 409 "second registration"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users/login" \
    --header 'Content-Type: application/json' \
    --header 'is_test: true' \
    --data-raw '{
    "username":"test@example.com",
    "password": "long_password"
}')

  check_response "$status" 404  "bad password"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users/login" \
    --header 'is_test: true' \
    --header 'Content-Type: application/json' \
    --data-raw '{
    "username":"testi@example.com",
    "password": "1223very long password!"
}')

  check_response "$status" 200  "login"
  
  token=$(jq -r .body <tmp.txt)
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" \
    --header "Authorization: $token" \
    --header 'is_test: true')
  check_response "$status" 200  "authenticated get all users"


  id=$(jq -r '.[0].id' <tmp.txt)
  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users/$id" -H 'is_test: true')
  check_response "$status" 401 "unauthenticated find one user"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users/$id" \
    --header 'is_test: true' \
    --header "Authorization: $token")
  
  check_response "$status" 200 "authenticated find one user"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$SERVER_URI/users/$id" \
    --header 'is_test: true')
  check_response "$status" 401 "unauthenticated delete"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$SERVER_URI/users/unique_id1184292312312321" \
    --header 'is_test: true' \
    --header "Authorization: $token")
  check_response "$status" 401 "authenticated delete of somebody else"

  status=$(curl -k -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$SERVER_URI/users/$id" \
    --header 'is_test: true' \
    --header "Authorization: $token")

  check_response "$status" 200 "authenticated delete"
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
