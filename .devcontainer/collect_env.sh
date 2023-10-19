#!/bin/bash
# This script is run every time a terminal is started. It does the following:
# it looks at .devcontainer/required_settings.json and updates a config file with the name/value pairs where the config
# filename is stored in $HOME with the name ".<project directory name>"  This file can then be used however the app
# needs -- typically passed into main via launch.json args[]

# in case $0 is modified somehow, store it in a local
SCRIPT_FQN="$0"
#colors for nice output
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
#   directory.  to make this work, we have to get the real paths to the required_settings.json and the actual
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

function get_config_file_name() {
    # Get the directory of the current script
    script_dir=$(dirname "$SCRIPT_FQN")

    # Navigate to the directory of the script and then two directories up
    #shellcheck disable=SC2164
    pushd "$script_dir/.." >/dev/null

    # Get the current directory name, which is now two levels up from the original script's directory
    project_name="$(basename "$PWD")"

    # Return to the original directory
    #shellcheck disable=SC2164
    popd >/dev/null

    file_path="$HOME/.$project_name.json"

    # Only create the file if it doesn't already exist
    [[ ! -f "$file_path" ]] && touch "$file_path"

    echo "$file_path"
}

# a this is a config file in json format where we use jq to find/store settings
# we pushd to the directory that has the script -- so it needs to be in the same directory as $0 (collect_env.sh)
DEVCONTAINER_DIR="$(dirname "${0}")"
REQUIRED_REPO_ENV_VARS="$DEVCONTAINER_DIR/required_settings.json"
CONFIG_FILE=$(get_config_file_name)


# update_config function
#

#   1.	load the required settings in $LOCAL_SETTINGS_FILE and iterate through its keys
#   2.	load the local settings from $CONFIG_FILE
#   3.	if the shellscript is not empty, it should "source" the script to get the new value
#   4.	if it is empty, it should prompt the user for the value using the description
#   5.	set the setting in the config file with the new value
#
function update_config() {
    local required_settings   # the settings in required_settings.json
    local value               # the value of the new setting
    local environmentVariable # name of the env variable if there's a script that gets the setting
    local description
    local shellscript
    local default
    local updated_settings # all the settings accumulated so far
    local existing_value

    if [[ "$CONFIG_FILE" -nt "$REQUIRED_REPO_ENV_VARS" ]]; then
        echo_info "using existing $CONFIG_FILE"
        return
    else
        echo_info "building config file $CONFIG_FILE"
    fi

    required_settings=$(jq '.' "$REQUIRED_REPO_ENV_VARS")
    existing_settings=$(jq '.' "$CONFIG_FILE")
    updated_settings="{}" # Initialize as an empty JSON object

    # Iterate through the array
    for key in $(echo "$required_settings" | jq -r 'keys[]'); do
        description=$(echo "$required_settings" | jq -r ".${key}.description")
        shellscript_line=$(echo "$required_settings" | jq -r ".${key}.shellscript")
        default=$(echo "$required_settings" | jq -r ".${key}.default")
        environmentVariable=$(echo "$required_settings" | jq -r ".${key}.tempEnvironmentVariableName")
        value=""

        # Check if the key already exists in the original settings
        existing_value=$(echo "$existing_settings" | jq -r ".${key}")

        # Get the script name by extracting everything before the first space
        shellscript="${shellscript_line%% *}"

        # Get the arguments by extracting everything after the first space
        script_args="${shellscript_line#* }"

        # Check to ensure the shellscript exists if it is set
        if [[ -n "$shellscript" && ! -f "$shellscript" ]]; then
            echo_error "ERROR: $shellscript specified in $REQUIRED_REPO_ENV_VARS does not exist."
            echo_error "$key will not be set."
            echo_error "Note:  \$PWD=$PWD"
            continue
        fi

        if [[ "$existing_value" != "null" ]]; then
            value="$existing_value" # If key exists, use its value.
        else
            if [[ -n "$shellscript" ]]; then
                chmod +x "$shellscript"
                #shellcheck disable=SC1090
                source "$shellscript" "$script_args"
                eval "value=\$$environmentVariable"
            fi

            if [[ -z "$value" ]]; then
                echo -n "Enter $description ($default): "
                read -r value
                if [[ -z "$value" ]]; then
                    value="$default"
                fi
                echo_warning "setting $key=$value"
            fi
        fi

        # Add/update the key-value pair in the JSON
        updated_settings=$(echo "$updated_settings" | jq --arg key "$key" --arg value "$value" '.[$key]=$value')
    done
    # Write the updated JSON back to the config file
    echo "$updated_settings" >"$CONFIG_FILE"

}

#
#   gets the fully qualified path to collect_env.sh and adds a line to the .bashrc or .zshrc to run
#   devscrets.sh update.  Also creates an empty required_settings.json file
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
    "setting": {
        "description": "this is the description of a sample setting",
        "shellscript": "",
        "default": "whatever you want",
        "tempEnvironmentVariableName": "the name of the env variable that your script sets for the setting"
    }' >"$REQUIRED_REPO_ENV_VARS"
    fi
}

function show_help() {
    echo "Usage: collect_env.sh [OPTIONS]"
    echo ""
    echo "OPTIONS:"
    echo "  help        Show this help message"
    echo "  update      parses required_settings.json and updates $LOCAL_REQUIRED_ENV_FILE"
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
    pushd "$DEVCONTAINER_DIR" >/dev/null ||
        {
            echo_error "Unable to change directory to $(dirname "$REQUIRED_REPO_ENV_VARS")"

        }
    update_config
    popd >/dev/null || {
        echo_error "can't popd"
    }
    ;;
setup)
    initial_setup

    ;;
reset)
    rm "$CONFIG_FILE" 2>/dev/null
    rm "$SSL_KEY_FILE" 2>/dev/null
    rm "$SSL_CERT_FILE" 2>/dev/null
    update_config
    ;;
*)
    echo "Invalid option: $1"
    show_help
    ;;
esac
