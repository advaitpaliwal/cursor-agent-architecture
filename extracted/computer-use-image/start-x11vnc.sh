#!/bin/bash

# Fast X server readiness check using socket
echo "Waiting for X server to be ready..."
for i in {1..50}; do
    if [ -S /tmp/.X11-unix/X99 ] && xdpyinfo -display :99 >/dev/null 2>&1; then
        echo "X server is ready!"
        # Start x11vnc immediately
        exec /usr/bin/x11vnc -display :99 -forever -shared -rfbport 5900 -rfbauth /root/.vnc/passwd -noxrecord -noxdamage
    fi
    sleep 0.1
done

echo "ERROR: X server did not become ready in time"
exit 1
