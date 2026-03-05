#!/bin/bash

# Fast X server readiness check using socket
echo "Waiting for X server..."
for i in {1..50}; do
    if [ -S /tmp/.X11-unix/X99 ]; then
        break
    fi
    sleep 0.1
done

# Read cached Chromium path
CHROMIUM_PATH=$(cat /usr/local/bin/chromium-path.txt 2>/dev/null)

# Fallback to search if cache is missing
if [ -z "$CHROMIUM_PATH" ] || [ ! -f "$CHROMIUM_PATH" ]; then
    echo "Cache miss, searching for Chromium binary..."
    CHROMIUM_PATH=$(find /root/.cache/ms-playwright -name "chrome" -type f 2>/dev/null | head -n1)
fi

if [ -z "$CHROMIUM_PATH" ]; then
    echo "ERROR: Could not find Chromium binary"
    exit 1
fi

echo "Starting Chromium from: $CHROMIUM_PATH"

# Start Chromium with optimized flags
exec "$CHROMIUM_PATH" \
    --no-sandbox \
    --disable-dev-shm-usage \
    --disable-gpu \
    --disable-software-rasterizer \
    --no-first-run \
    --no-default-browser-check \
    --disable-background-networking \
    --disable-sync \
    --disable-translate \
    --disable-extensions \
    --disable-features=TranslateUI \
    --disable-popup-blocking \
    --start-maximized \
    --window-position=0,0 \
    --window-size=1920,1080 \
    --log-level=3 \
    --test-type \
    "https://www.google.com"
