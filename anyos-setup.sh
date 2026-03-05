#!/bin/bash
# AnyOS Configuration Setup Script
# =================================
# Applies configuration from anyos.conf to all config files.
# Run this during Docker build after copying config templates.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF_FILE="${SCRIPT_DIR}/anyos.conf"

# Source configuration
if [ -f "$CONF_FILE" ]; then
    # Extract variable assignments (ignore comments and empty lines)
    while IFS='=' read -r key value; do
        # Skip comments and empty lines
        [[ "$key" =~ ^#.*$ ]] && continue
        [[ -z "$key" ]] && continue
        # Remove leading/trailing whitespace
        key=$(echo "$key" | xargs)
        value=$(echo "$value" | xargs)
        # Export the variable
        export "$key=$value"
    done < "$CONF_FILE"
else
    echo "ERROR: anyos.conf not found at $CONF_FILE"
    exit 1
fi

# Computed values
export ANYOS_RESOLUTION="${ANYOS_DISPLAY_WIDTH}x${ANYOS_DISPLAY_HEIGHT}"
export ANYOS_VNC_RESOLUTION="${ANYOS_RESOLUTION}x${ANYOS_DISPLAY_DEPTH}"
export ANYOS_UI_FONT="${ANYOS_FONT_NAME} ${ANYOS_FONT_SIZE}"
export ANYOS_TERMINAL_FONT="${ANYOS_TERMINAL_FONT_NAME} ${ANYOS_TERMINAL_FONT_SIZE}"
export ANYOS_TITLE_FONT="${ANYOS_TITLE_FONT_NAME} ${ANYOS_TITLE_FONT_SIZE}"

echo "AnyOS Configuration:"
echo "  Resolution: ${ANYOS_RESOLUTION} @ ${ANYOS_DPI} DPI"
echo "  Panel: ${ANYOS_PANEL_HEIGHT}px, Dock icons: ${ANYOS_DOCK_ICON_SIZE}px"
echo "  UI Font: ${ANYOS_UI_FONT}"

# Function to substitute placeholders in a file
substitute_file() {
    local file="$1"
    if [ -f "$file" ]; then
        sed -i \
            -e "s/ANYOS_DISPLAY_WIDTH/${ANYOS_DISPLAY_WIDTH}/g" \
            -e "s/ANYOS_DISPLAY_HEIGHT/${ANYOS_DISPLAY_HEIGHT}/g" \
            -e "s/ANYOS_DISPLAY_DEPTH/${ANYOS_DISPLAY_DEPTH}/g" \
            -e "s/ANYOS_DPI/${ANYOS_DPI}/g" \
            -e "s/ANYOS_FRAMERATE/${ANYOS_FRAMERATE}/g" \
            -e "s/ANYOS_PANEL_HEIGHT/${ANYOS_PANEL_HEIGHT}/g" \
            -e "s/ANYOS_DOCK_ICON_SIZE/${ANYOS_DOCK_ICON_SIZE}/g" \
            -e "s/ANYOS_CURSOR_SIZE/${ANYOS_CURSOR_SIZE}/g" \
            -e "s/ANYOS_FONT_SIZE/${ANYOS_FONT_SIZE}/g" \
            -e "s/ANYOS_TERMINAL_FONT_SIZE/${ANYOS_TERMINAL_FONT_SIZE}/g" \
            -e "s/ANYOS_TITLE_FONT_SIZE/${ANYOS_TITLE_FONT_SIZE}/g" \
            -e "s/ANYOS_FONT_NAME/${ANYOS_FONT_NAME}/g" \
            -e "s/ANYOS_TERMINAL_FONT_NAME/${ANYOS_TERMINAL_FONT_NAME}/g" \
            -e "s/ANYOS_TITLE_FONT_NAME/${ANYOS_TITLE_FONT_NAME}/g" \
            -e "s/ANYOS_RESOLUTION/${ANYOS_RESOLUTION}/g" \
            -e "s/ANYOS_VNC_RESOLUTION/${ANYOS_VNC_RESOLUTION}/g" \
            "$file"
        echo "  Configured: $file"
    fi
}

# Apply to all config files in target directory
apply_config() {
    local target_dir="$1"
    echo "Applying AnyOS configuration to $target_dir..."

    # X resources
    substitute_file "$target_dir/.Xresources"

    # XFCE settings
    substitute_file "$target_dir/.config/xfce4/xfconf/xfce-perchannel-xml/xsettings.xml"
    substitute_file "$target_dir/.config/xfce4/xfconf/xfce-perchannel-xml/xfce4-panel.xml"
    substitute_file "$target_dir/.config/xfce4/xfconf/xfce-perchannel-xml/xfwm4.xml"
    substitute_file "$target_dir/.config/xfce4/terminal/terminalrc"

    # GTK settings
    substitute_file "$target_dir/.config/gtk-3.0/settings.ini"

    # Plank dock
    substitute_file "$target_dir/.config/plank/dock1/settings"
}

# If called with a directory argument, apply config there
if [ -n "$1" ]; then
    apply_config "$1"
fi
