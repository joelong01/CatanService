#!/bin/bash

# Iterate over all provided file arguments
for file in "$@"; do
    if [[ -f "$file" ]]; then
        while IFS= read -r line; do
            # Skip any lines that don't look like variable assignments
            if [[ "$line" =~ ^[a-zA-Z_][a-zA-Z0-9_]*= ]]; then
                var_name="${line%%=*}"
                var_value="${line#*=}"
                export "$var_name=$var_value"
            fi
        done < "$file"
    else
        echo "Warning: File '$file' not found."
    fi
done
