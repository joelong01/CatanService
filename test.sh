#!/bin/bash
PASS=0
FAIL=0
RED=$(tput setaf 1)
GREEN=$(tput setaf 2)
YELLOW=$(tput setaf 3)
SERVER_URI="http://localhost:8080/api/v1"
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
  if [[ $VERBOSE ]]; then
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

create_test_users() {
  for i in {1..5}; do
    cat <<EOF
{
  "partition_key": 1,
  "id": "$(uuidgen)",
  "password_hash": null,
  "password": "long_password_that_is_ a test $i",
  "email": "test$i@example.com",
  "first_name": "Test$i",
  "last_name": "User$i",
  "display_name": "Test User$i",
  "picture_url": "https://example.com/pic$i.jpg",
  "foreground_color": "#00000$i",
  "background_color": "#FFFFFF$i",
  "games_played": $((10 * i)),
  "games_won": $((5 * i))
}
EOF
  done
}

## check_response: $1 is the return from curl
#                  $2 is the expected retrun from curl (if we wanted to do negative tests)
#                  $3 is the expected JSON response
#
# tmp.txt contains the actual return from the curl call, which is always in JSON format for this WebAPI
#
# the JSON response contains the StatusCode, which should match what curl got (unless their is a connectivity problem)
# if we are verbose, we can echo the response

check_response() {
  curl_status=$1
  curl_expected_status=$2
  expected_content=$3
  content=$(cat tmp.txt)

  if [[ "$curl_status" -eq $curl_expected_status && $content == *"$expected_content"* ]]; then
    echo_if_verbose "$content"
    ((PASS++))
  else
    echo_error "expected $expected_content got $content"
    echo_error "FAIL"
    ((FAIL++))
  fi
}

function run_tests() {
  echo_warning "Running setup on the database"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location --request POST "$SERVER_URI/setup"  -H 'is_test: true' )
  check_response "$status" 200 "database: Users-db collection: User-Container"

  echo_warning "Looking for Users. This should be empty:"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" -H 'is_test: true' )
  check_response "$status" 200 "[]"

  echo_warning "Creating user"

 

    json=$(
      cat <<EOF
{
  "partition_key": 1,
  "id": "",
  "password_hash": "",
  "password": "long_password_that_is_ a test $i",
  "email": "test$i@example.com",
  "first_name": "Doug ",
  "last_name": "Smit",
  "display_name": "Dougy",
  "picture_url": "https://hagadone.media.clients.ellingtoncms.com/img/photos/2021/04/27/DougSmith_-_1_tx658.jpg?fabbae1045d0968743dc2748279f625e68141661",
  "foreground_color": "#000000",
  "background_color": "#FFFFFFF",
  "games_played": 10,
  "games_won": 1
}
EOF
    )

    status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" \
      -H 'Content-Type: application/json' \
      -H 'is_test: true' \
      -d "$json")
    check_response "$status" 200 "id"


  echo_warning "Getting all users again"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" -H 'is_test: true' )
  check_response "$status" 200 "$user"

  echo_warning "Finding one user"
  id=$(echo "$user" | jq -r .id)
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users/$id" -H 'is_test: true' )
  found_user=$(cat tmp.txt)
  echo_if_verbose "$found_user \n $status"

  echo_warning "Deleting the user"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$SERVER_URI/users/$id" -H 'is_test: true' )
  check_response "$status" 200 "deleted user with id: $id"
}

function print_results() {
  echo_info "PASS: $PASS"
  if [[ $FAIL -gt 0 ]]; then
    echo_error "FAIL: $FAIL"
  else
    echo_info "FILE: 0"
  fi

}

function clean_up() {
  rm tmp.txt 2>/dev/null
}

parse_input
run_tests
print_results
clean_up
