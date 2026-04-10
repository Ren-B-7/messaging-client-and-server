#!/bin/bash

# Usage: ./toggle_env.sh [full|min]

ENV=$1

if [[ "$ENV" != "full" && "$ENV" != "min" ]]; then
    echo "Usage: ./toggle_env.sh [full|min]"
    exit 1
fi

echo "Switching frontend environment to: $ENV"

# 1. Update platform.config.js
sed -i "s/ENV: \".*\"/ENV: \"$ENV\"/" frontend/static/js/full/config/platform.config.js

# 2. Update all HTML files (links and modulepreloads)
# Use explicit patterns for paths to avoid unintended replacements.
if [[ "$ENV" == "min" ]]; then
    # Full -> Min
    find frontend -name "*.html" -exec sed -i 's|non-static/full/|non-static/min/|g' {} +
    find frontend -name "*.html" -exec sed -i 's|static/js/full/|static/js/min/|g' {} +
    find frontend -name "*.html" -exec sed -i 's|static/css/full/|static/css/min/|g' {} +
else
    # Min -> Full
    find frontend -name "*.html" -exec sed -i 's|non-static/min/|non-static/full/|g' {} +
    find frontend -name "*.html" -exec sed -i 's|static/js/min/|static/js/full/|g' {} +
    find frontend -name "*.html" -exec sed -i 's|static/css/min/|static/css/full/|g' {} +
fi

echo "Environment switched to $ENV. Please clear browser cache."
