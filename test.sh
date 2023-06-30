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
      u )
        SERVER_URI=$OPTARG
        ;;
      v )
        VERBOSE=true
        ;;
      \? )
        echo "Invalid Option: -$OPTARG" 1>&2
        exit 1
        ;;
      : )
        echo "Invalid Option: -$OPTARG requires an argument" 1>&2
        exit 1
        ;;
    esac
  done
  shift $((OPTIND -1))
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
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location --request POST "$SERVER_URI/setup")
  check_response "$status" 200 "database: Users-db collection: User-Container"


  echo_warning "Looking for Users. This should be empty:"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users")
  check_response "$status" 200 "[]"


  echo_warning "Creating a user"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users" \
  --header 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'name=doug' \
  --data-urlencode 'email=dougo@test.com')
  user=$(cat tmp.txt)
  check_response "$status" 200 "id"


  echo_warning "Getting all users again"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users")
  check_response "$status" 200 "$user"

  echo_warning "Finding one user"
  id=$(echo "$user" | jq -r .id)
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location "$SERVER_URI/users/$id")
  found_user=$(cat tmp.txt)
  echo_if_verbose "$found_user \n $status"


  echo_warning "Deleting the user"
  status=$(curl -s -w "%{http_code}" -o tmp.txt --location --request DELETE "$SERVER_URI/users/$id")
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