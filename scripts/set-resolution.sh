#!/bin/bash
# AnyOS Resolution Helper
# =======================
# Change the display resolution dynamically.
# Requires anyos.conf to be present.

# Source AnyOS config (required)
ANYOS_CONF="/usr/local/share/anyos.conf"
if [ ! -f "$ANYOS_CONF" ]; then
    echo "ERROR: AnyOS config not found at $ANYOS_CONF"
    exit 1
fi

while IFS='=' read -r key value; do
    [[ "$key" =~ ^#.*$ ]] && continue
    [[ -z "$key" ]] && continue
    key=$(echo "$key" | xargs)
    value=$(echo "$value" | xargs)
    export "$key=$value"
done < "$ANYOS_CONF"

RESOLUTION=${1:-${ANYOS_DISPLAY_WIDTH}x${ANYOS_DISPLAY_HEIGHT}}
DPI=${2:-${ANYOS_DPI}}

xrandr --fb ${RESOLUTION} --dpi ${DPI} 2>/dev/null
if [ $? -eq 0 ]; then
    echo "AnyOS resolution set to ${RESOLUTION} at ${DPI} DPI"
else
    echo "Failed to set resolution"
    exit 1
fi
