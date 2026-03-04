#!/bin/bash
# Install HiDPI xfwm4 window decoration assets for WhiteSur theme.
# Run this AFTER the main vnc-desktop.yml playbook when using 4K @ 2x DPI.
#
# The standard WhiteSur theme has 24x28px button assets which look tiny at 2x.
# This script downloads and overlays 36x42px HiDPI assets from the source repo.

set -euo pipefail

echo "Downloading WhiteSur GTK theme source..."
cd /tmp
rm -rf whitesur-src whitesur-src.tar.gz

# Download with retry
for i in 1 2 3 4 5; do
    if curl -fsSL -o whitesur-src.tar.gz \
        "https://api.github.com/repos/vinceliuice/WhiteSur-gtk-theme/tarball/master"; then
        break
    fi
    echo "Download attempt $i failed, retrying in 10s..."
    sleep 10
done

echo "Extracting..."
mkdir -p whitesur-src
tar -xzf whitesur-src.tar.gz -C whitesur-src --strip-components=1

echo "Overlaying HiDPI xfwm4 assets..."
cp -f whitesur-src/src/assets/xfwm4/assets-Light-hdpi/* /usr/share/themes/WhiteSur-Light/xfwm4/

echo "Cleaning up..."
rm -rf whitesur-src whitesur-src.tar.gz

echo "Done. HiDPI window decoration assets installed."
