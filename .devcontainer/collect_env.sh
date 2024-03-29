#!/bin/bash
# This script is run every time a terminal is started. It does the following:
# 1. Load the local environment from local.env
# 2. Login to GitHub with the proper scope
# 3. Login to Azure, optionally with a service principal
# 4. Setup the environment

# this protects the readonly variables from trying to be set again -- this would happen if the
# dev runs ./collect_env.sh reset

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
#   it is an easy mistake to run ./collect_env.sh setup from the devcontainer directory instead of the project
#   directory.  to make this work, we have to get the real paths to the required-env.json and the actual
#   script itself
function get_real_path() {
    local file
    file=$1
    if [[ -f $file ]]; then
        realpath "$file"
        return 0
    fi

    file=".devcontainer/$file"
    if [[ -f $file ]]; then
        realpath "$file"
        return 0
    fi
    echo_error "can't find $file. Run $0 from the project root directory"
    exit 1
}

# a this is a config file in json format where we use jq to find/store settings
# we pushd to the directory that has the script -- so it needs to be in the same directory as $0 (collect_env.sh)
REQUIRED_REPO_ENV_VARS="./required-env.json"

#this is where we put the environment variables to be loaded by the shell
LOCAL_REQUIRED_ENV_FILE="$HOME/.localDevEnv.sh"
USE_GITHUB_USER_SECRETS=$(jq -r '.options.useGitHubUserSecrets' "$REQUIRED_REPO_ENV_VARS" 2>/dev/null)

# collect_env function
#
#   $1 contains a JSON array
#   1.	iterate through the array
#   2.	check a file called $LOCAL_REQUIRED_ENV_FILE to see if there is text in the form “KEY=VALUE”
#       where KEY is the environment variable name (use sed for this)
#           a.	if the value is set in the file, set an environment variable with this key and value
#           b.	continue to the next array element
#   3.	if the shellscript is not empty, it should "source" the script
#   4.	if it is empty, it should prompt the user for the value using the description
#   5.	it should set the environment variable to the value entered
#   6.	if the script is called, the script will set the environment variable
#
function collect_env() {
    # Parse the JSON input
    local json_array
    local length # the length of the json array
    local value
    local environmentVariable # the name of the environment variable
    local description
    local shellscript # a shellscript that will provide information to the user to collect the needed value
    local default

    json_array=$(jq '.secrets' "$REQUIRED_REPO_ENV_VARS")
    length=$(echo "$json_array" | jq 'length')
    # Iterate through the array
    for ((i = 0; i < length; i++)); do
        # Extract JSON properties
        environmentVariable=$(echo "$json_array" | jq -r ".[$i].environmentVariable")
        description=$(echo "$json_array" | jq -r ".[$i].description")
        shellscript_line=$(echo "$json_array" | jq -r ".[$i].shellscript")
        default=$(echo "$json_array" | jq -r ".[$i].default")

        # Get the script name by extracting everything before the first space
        shellscript="${shellscript_line%% *}"

        # Get the arguments by extracting everything after the first space
        script_args="${shellscript_line#* }"

        # check to make sure that if shellscript is set that the file exists
        if [[ -n "$shellscript" && ! -f "$shellscript" ]]; then
            echo_error "ERROR: $shellscript specified in $REQUIRED_REPO_ENV_VARS does not exist."
            echo_error "$environmentVariable will not be set."
            echo_error "Note:  \$PWD=$PWD"
            continue
        fi

        # Check if the environment variable is set in the local secrets file

        value=$(grep "^$environmentVariable=" "$LOCAL_REQUIRED_ENV_FILE" 2>/dev/null | sed 's/^[^=]*=//')
        value="${value%\"}" # Remove trailing quote
        value="${value#\"}" # Remove leading quote

        if [[ -n "$value" ]]; then
            # Get the value from the secret_entry
            # Set the environment variable with the key and value from the file
            export "$environmentVariable"="$value"

        else
            if [[ -n "$shellscript" ]]; then
                # make sure the file is executable
                chmod +x "$shellscript"
                # If shellscript is not empty, source it
                #shellcheck disable=SC1090
                source "$shellscript" "$script_args"
                eval "value=\$$environmentVariable"

            fi
            if [[ -z "$value" ]]; then #if the script doesn't set the value, prompt for it.
                # If shellscript is empty, prompt the user for the value using the description and the default
                echo -n "Enter $description ($default): "
                read -r value

                # Set the environment variable to the value entered

                if [[ -n $value ]]; then
                    echo_warning "setting $environmentVariable=$value"
                    export "$environmentVariable=$value"
                else
                    echo_warning "setting $environmentVariable=$default"
                    export "$environmentVariable=$default"
                fi
            fi
        fi
    done
}

# build_set_env_script function
# this builds the script that is called by update_secrets.sh that sets the secrets
# $1 contains a JSON array

function build_set_env_script() {

    # load the secrets file to get the array of secrets
    local json_array # a json array of secrets loaded from the $REQUIRED_REPO_ENV_VARS file
    local toWrite=""
    local environmentVariable
    local description
    local val # the value of the environment variable

    local length # the length of the json array

    json_array=$(jq '.secrets' "$REQUIRED_REPO_ENV_VARS")

    # found a bug in the VS Code shell beautify where making this a string that gets appended to
    # breaks the formatting. this form does not.
    cat <<EOF >"$LOCAL_REQUIRED_ENV_FILE"
#!/bin/bash
# if we are running in codespaces, we don't load the local environment
if [[ \$CODESPACES == true ]]; then
  return 0
fi
EOF

    length=$(echo "$json_array" | jq '. | length')
    # Iterate through the array
    for ((i = 0; i < length; i++)); do
        environmentVariable=$(echo "$json_array" | jq -r ".[$i].environmentVariable")
        # using eval as zsh would sometimes give "bad substitution when using parameter expansion
        eval val="\$$environmentVariable"
        val="${val//\"/\\\"}" # this escapes any quotes
        description=$(echo "$json_array" | jq -r ".[$i].description")
        toWrite+="# $description\nexport $environmentVariable\n$environmentVariable=\"$val\"\n"
    done
    echo -e "$toWrite" >>"$LOCAL_REQUIRED_ENV_FILE"
    # we don't have to worry about sourcing this when in CodeSpaces as the script will exit if
    # CODESPACES == true.  the shellcheck disable is there to tell the linter to not worry about
    # linting the script that we are sourcing
    # shellcheck disable=1090
    source "$LOCAL_REQUIRED_ENV_FILE"
}

# function save_in_codespaces()
# $1 contains a JSON array of secrets
# go through that array and save the environment variable in Codespaces User Secrets
# makes sure that if the environment variable already exists *add* the current repo to the repo list instead of just updating it
# which in current GitHub, resets the environment variable to be valid in only the specified repos
# assumes that every environment variable has an environment variable set with the correct value
function save_in_codespaces() {

    local repos               # the repos the environment variable is available in
    local url                 # the url to get the environment variable's repos
    local environmentVariable # the name of the environment variable
    local val                 # the secrets value
    local gh_pat              # the GitHub PAT - needed to call the REST api
    local current_repo        # the repo of the current project
    local json_array          # renamed $1: a json array of secrets (environmentVariable, description, and shellscript)
    local length              # the length of the json array

    json_array=$1
    length=$(echo "$json_array" | jq '. | length')
    current_repo=$(git config --get remote.origin.url | sed -e 's|^https://github.com/||' | sed -e 's|.git$||')

    gh_pat=$(gh auth token)
    length=$(echo "$json_array" | jq '. | length')
    for ((i = 0; i < length; i++)); do
        environmentVariable=$(echo "$json_array" | jq -r ".[$i].environmentVariable")
        eval "val=\"\$${environmentVariable}\"" # eval can be used in both bash and zsh to perform indirect reference
        url="https://api.github.com/user/codespaces/secrets/$environmentVariable/repositories"

        # this curl syntax will allow us to get the resonse and the response code
        response=$(curl -s -w "%{http_code}" -H "Authorization: Bearer $gh_pat" "$url")
        response_code=${response: -3}
        response=${response:0:${#response}-3}

        # if the secret is not set, we'll get a 404 back.  then the repo is just the current repo
        case $response_code in
        "404")
            repos="$current_repo"
            ;;
        "200")
            # a 2xx indicates that the user secret already exists.  get the repos that the secret is valid in.
            repos=$(echo "$response" | jq '.repositories[].full_name' | paste -sd ",")
            # Check if current_repo already exists in repos, and if not then add it
            # if you don't do this, the gh secret set api will give an error
            if [[ $repos != *"$current_repo"* ]]; then
                repos+=",\"$current_repo\""
            fi
            ;;
        *)
            echo_error "unknown error calling $url"
            echo_error "Secret=$environmentVariable value=$val in repos=$repos"
            ;;
        esac

        # set the secret -- we always do this as the value might have changed...we can't check the value
        # using the current GH api.
        gh secret set "$environmentVariable" --user --app codespaces --repos "$repos" --body "$val"
    done
}

#
#   load the required-env.json file and for each environment variable specified do
#   1. check if the value is known, if not prompt the user for the value
#   2. reconstruct and overwrite the $LOCAL_REQUIRED_ENV_FILE
#   3. source the $LOCAL_REQUIRED_ENV_FILE
function update_vars {
    # check the last modified date of the env file file and if it is gt the last modified time of the config file
    # we have no work to do
    local local_file_modified
    local required_file_modified

    if [[ -f "$LOCAL_REQUIRED_ENV_FILE" ]]; then
        if [[ $(uname) == "Darwin" ]]; then
            local_file_modified=$(stat -f "%m" "$LOCAL_REQUIRED_ENV_FILE")
            required_file_modified=$(stat -f "%m" "$REQUIRED_REPO_ENV_VARS")
        else
            local_file_modified=$(stat -c "%Y" "$LOCAL_REQUIRED_ENV_FILE")
            required_file_modified=$(stat -c "%Y" "$REQUIRED_REPO_ENV_VARS")
        fi
    else
        # force the creation of the file - this makes the below if statment false
        local_file_modified=0
        required_file_modified=1
    fi

    if [[ $local_file_modified -ge $required_file_modified ]]; then
        echo_info "Using existing $LOCAL_REQUIRED_ENV_FILE"
        echo_info "Update $REQUIRED_REPO_ENV_VARS if you want more secrets!"
        #shellcheck disable=SC1090
        source "$LOCAL_REQUIRED_ENV_FILE"
        return 0
    fi

    # we require GitHub login if GitHub secrets are being used. there might be other reasons outside the pervue of this
    # this script to login to GitHub..
    if [[ $USE_GITHUB_USER_SECRETS == "true" ]]; then
        login_to_github
    fi

    # iterate through the JSON and get values for each secret
    # when this returns each secret will have an environment variable set

    collect_env
    build_set_env_script



    #TODO:  if you want to support codespace -- add the call to update the secrets here

}

# see if the user is logged into GitHub with the scopes necessary to use Codespaces secrets.
#  not, log them in.
function login_to_github() {

    export GH_AUTH_STATUS
    GH_AUTH_STATUS=$(gh auth status 2>&1)

    # there are three interesting cases coming back in GH_AUTH_STATUS
    # 1. logged in with the correct scopes
    # 2. logged in, but with the wrong scopes
    # 3. not logged in.
    # here we deal with all 3 of those possibilities
    if [[ "$GH_AUTH_STATUS" == *"not logged into"* ]]; then
        USER_LOGGED_IN=false
    else
        USER_LOGGED_IN=true
    fi

    # find the number of secrets to test if we have the write scopes for our github login
    # if they are not logged in, this fails and SECRET_COUNT is empty
    SECRET_COUNT=$(gh api -H "Accept: application/vnd.github+json" /user/codespaces/secrets | jq -r .total_count)

    # without secrets, the SECRET_COUNT is 0 if they are logged in with the right permissions. so if it is empty...
    if [[ -z $SECRET_COUNT ]] && [[ $USER_LOGGED_IN == true ]]; then
        echo_warning "Refreshing GitHub Token to request codespace:secrets scope"
        gh auth refresh --scopes user,repo,codespace:secrets
    fi

    # ""You are not logged into any GitHub hosts. Run gh auth login to authenticate.""
    # is the message returned for gh auth status when the user isn't signed in
    # it is possible that github could change this in the future, which would break
    # this script, so "not logged into" seems like a safer thing to check.
    if [[ $USER_LOGGED_IN == false ]]; then
        gh auth login --scopes user,repo,codespace:secrets
        GH_AUTH_STATUS=$(gh auth status 2>&1)
    fi

    # this just echos a nice message to the user...GitLabs should have a --json option for this!
    # we also *want* expansion/globbing here to find the check, so disable SC2086 for this one line
    #shellcheck disable=SC2086
    GITHUB_ACCOUNT_NAME="$(echo "$GH_AUTH_STATUS" | sed -n -e 's/^.*Logged in to github.com as \([^ ]*\) .*/\1/p')"
    if [[ -z ${GITHUB_ACCOUNT_NAME} ]]; then
        echo_warning "You are not logged into GitHub"
    else
        echo_info "You are logged in to Github.com as $GITHUB_ACCOUNT_NAME"
    fi
}
#
#   gets the fully qualified path to collect_env.sh and adds a line to the .bashrc or .zshrc to run
#   devscrets.sh update.  Also creates an empty required-env.json file
function initial_setup() {
    # Define the startup line to be added to the .bashrc
    local startup_line
    local this_script
    this_script=$(get_real_path collect_env.sh)

    startup_line="source $this_script update"
    # Check if the startup line exists in the .bashrc file
    if ! grep -q "${startup_line}" "$HOME"/.bashrc; then
        # If it doesn't exist, append the line to the .bashrc file
        echo "${startup_line}" >>"$HOME"/.bashrc
    fi

    # Check if the startup line exists in the .zshrc file
    if ! grep -q "${startup_line}" "$HOME"/.zshrc; then
        # If it doesn't exist, append the line to the .bashrc file
        echo "${startup_line}" >>"$HOME"/.zshrc
    fi

    # if there isn't a json file, create a default one
    if [[ ! -f $REQUIRED_REPO_ENV_VARS ]]; then
        echo '{
    "options": {
        "useGitHubUserSecrets": false
    },
    "secrets": []
        }' >"$REQUIRED_REPO_ENV_VARS"
    fi
}

function show_help() {
    echo "Usage: collect_env.sh [OPTIONS]"
    echo ""
    echo "OPTIONS:"
    echo "  help        Show this help message"
    echo "  update      parses required-env.json and updates $LOCAL_REQUIRED_ENV_FILE"
    echo "  setup       modifies the devcontainer.json to bootstrap the system"
    echo "  reset       Resets $LOCAL_REQUIRED_ENV_FILE and runs update"
    echo ""
}
# this is where code execution starts
case "$1" in
help)
    show_help
    ;;
update)
    echo_info "running update"
    devcontainer_dir="$(dirname "$0")"
    pushd "$devcontainer_dir" >/dev/null ||
        {
            echo_error "Unable to change directory to $(dirname "$REQUIRED_REPO_ENV_VARS")"
            exit
        }
    update_vars
    popd >/dev/null || exit
    ;;
setup)
    initial_setup

    ;;
reset)
    rm "$LOCAL_REQUIRED_ENV_FILE" 2>/dev/null
    rm "$SSL_KEY_FILE" 2>/dev/null
    rm "$SSL_CERT_FILE" 2>/dev/null
    update_vars
    ;;
*)
    echo "Invalid option: $1"
    show_help
    ;;
esac
