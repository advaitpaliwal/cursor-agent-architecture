#!/bin/bash
# AnyOS Desktop Environment Entrypoint
# =====================================
# Initializes the AnyOS desktop environment.

# Source AnyOS configuration for display and scaling settings
ANYOS_CONF="/usr/local/share/anyos.conf"
if [ ! -f "$ANYOS_CONF" ]; then
    echo "ERROR: AnyOS config not found at $ANYOS_CONF"
    exit 1
fi
# shellcheck source=/dev/null
source "$ANYOS_CONF"

user_name="${USERNAME:-ubuntu}"
group_name="$(id -gn ${user_name})"
LOG=/tmp/container-init.log

VNC_PORT="${VNC_PORT:-5901}"
NOVNC_PORT="${NOVNC_PORT:-26058}"

# Core environment (from anyos.conf)
export DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-"autolaunch:"}"
export DISPLAY="${DISPLAY:-:1}"
export VNC_RESOLUTION="${VNC_RESOLUTION:-${ANYOS_DISPLAY_WIDTH}x${ANYOS_DISPLAY_HEIGHT}x${ANYOS_DISPLAY_DEPTH}}"
export VNC_DPI="${VNC_DPI:-${ANYOS_DPI}}"
export LANG="${LANG:-"en_US.UTF-8"}"
export LANGUAGE="${LANGUAGE:-"en_US.UTF-8"}"

# Software rendering for WebGL support in VNC (no hardware GPU)
# This enables Mesa's llvmpipe software renderer for OpenGL/WebGL
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe

# GTK/Qt environment (from anyos.conf - no fallbacks, must exist)
export GDK_SCALE="${ANYOS_GDK_SCALE}"
export GDK_DPI_SCALE="${ANYOS_GDK_DPI_SCALE}"
export QT_SCALE_FACTOR="${ANYOS_QT_SCALE_FACTOR}"
export XCURSOR_SIZE="${ANYOS_CURSOR_SIZE_PHYSICAL}"
# Electron apps (Cursor, VSCode, etc.)
export ELECTRON_FORCE_IS_PACKAGED=0
export ELECTRON_FORCE_DEVICE_SCALE_FACTOR="${ANYOS_GDK_SCALE}"

log() { echo -e "[$(date)] $@" | tee -a $LOG; }
log_error() { echo -e "[$(date)] ERROR: $@" | tee -a $LOG >&2; }

sudoIf() {
    if [ "$(id -u)" -ne 0 ]; then sudo "$@"; else "$@"; fi
}

sudoUserIf() {
    if [ "$(id -u)" -eq 0 ] && [ "${user_name}" != "root" ]; then
        sudo -u ${user_name} "$@"
    else
        "$@"
    fi
}

# Debug helpers for service failures
dump_dbus_debug() {
    log_error "=== D-Bus Debug Info ==="
    log_error "dbus-daemon processes: $(pgrep -a dbus-daemon 2>&1 || echo 'none')"
    log_error "/var/run/dbus/pid exists: $([ -f /var/run/dbus/pid ] && cat /var/run/dbus/pid || echo 'no')"
    log_error "dbus log tail: $(tail -80 /tmp/dbus-daemon-system.log 2>&1 || echo 'no log')"
    log_error "systemctl dbus status: $(systemctl status dbus 2>&1 || echo 'systemctl not available')"
}

dump_vnc_debug() {
    log_error "=== VNC Debug Info ==="
    log_error "tigervnc processes: $(pgrep -a tigervnc 2>&1 || echo 'none')"
    log_error "Xtigervnc processes: $(pgrep -a Xtigervnc 2>&1 || echo 'none')"
    log_error "VNC log: $(cat ~/.vnc/*.log 2>&1 | tail -80 || echo 'no vnc log')"
    log_error "X11 socket dir: $(ls -la /tmp/.X11-unix/ 2>&1 || echo 'no X11 socket dir')"
    log_error "X lock files: $(ls -la /tmp/.X*-lock 2>&1 || echo 'no lock files')"
    log_error "DISPLAY=$DISPLAY, VNC_PORT=$VNC_PORT"
}

dump_x_debug() {
    log_error "=== X Server Debug Info ==="
    log_error "DISPLAY=$DISPLAY"
    log_error "xdpyinfo output: $(sudoUserIf xdpyinfo -display "${DISPLAY}" 2>&1 | head -20 || echo 'xdpyinfo failed')"
    log_error "X11 socket: $(ls -la /tmp/.X11-unix/ 2>&1)"
    log_error "Xtigervnc processes: $(pgrep -a Xtigervnc 2>&1 || echo 'none')"
    log_error "VNC log tail: $(cat ~/.vnc/*.log 2>&1 | tail -80 || echo 'no vnc log')"
    log_error "xfce4 processes: $(pgrep -a xfce 2>&1 || echo 'none')"
}

dump_docker_debug() {
    log_error "=== Docker Debug Info ==="
    log_error "docker socket: $(ls -la /var/run/docker.sock 2>&1 || echo 'no socket')"
    log_error "dockerd processes: $(pgrep -a dockerd 2>&1 || echo 'none')"
    log_error "containerd processes: $(pgrep -a containerd 2>&1 || echo 'none')"
    log_error "docker service log: $(tail -80 /tmp/docker-service.log 2>&1 || echo 'no log')"
    log_error "docker info: $(docker info 2>&1 | head -20 || echo 'docker info failed')"
}

# Wait for window manager to be ready (XFCE4's xfwm4)
# xdpyinfo succeeding only means X server accepts connections,
# not that the window manager is initialized
wait_for_window_manager() {
    local max_attempts=60  # 30 seconds at 0.5s intervals
    local attempts=0

    # Check using EWMH standard property (_NET_SUPPORTING_WM_CHECK)
    while ! sudoUserIf xprop -root _NET_SUPPORTING_WM_CHECK -display "${DISPLAY}" 2>/dev/null | grep -q "window id"; do
        sleep 0.5
        attempts=$((attempts + 1))
        if [ $attempts -ge $max_attempts ]; then
            log_error "Window manager not detected via EWMH after 30 seconds"
            return 1
        fi
    done
    return 0
}

log "** AnyOS DESKTOP INIT START **"
log "Resolution: ${VNC_RESOLUTION}, DPI: ${VNC_DPI}, GDK_SCALE: ${GDK_SCALE}"

log "Redirecting core dumps to /tmp"
sudoIf sysctl -w kernel.core_pattern="/tmp/core.%e.%p" > /dev/null 2>&1 || true

# === PHASE 2: Start D-Bus (required for service command and XFCE) ===
log "Starting D-Bus daemon..."
if [ -f "/var/run/dbus/pid" ] && ! pgrep -x dbus-daemon > /dev/null; then
    sudoIf rm -f /var/run/dbus/pid
fi
dbus_output=$(sudoIf /etc/init.d/dbus start 2>&1)
dbus_exit=$?
echo "$dbus_output" | tee -a /tmp/dbus-daemon-system.log
if [ $dbus_exit -ne 0 ]; then
    log_error "D-Bus start command failed with exit code $dbus_exit"
    dump_dbus_debug
fi

# Wait for D-Bus daemon to be running (timeout: 30s)
dbus_attempts=0
while ! pgrep -x dbus-daemon > /dev/null; do
    sleep 0.5
    dbus_attempts=$((dbus_attempts + 1))
    if [ $dbus_attempts -ge 60 ]; then
        log_error "D-Bus daemon failed to start after 30 seconds"
        dump_dbus_debug
        break
    fi
done
if pgrep -x dbus-daemon > /dev/null; then
    log "D-Bus daemon is running"
else
    log_error "D-Bus daemon is NOT running - continuing anyway"
fi


# Setup X11 socket directory
log "Setting up X11 socket directory..."
sudoIf rm -rf /tmp/.X11-unix /tmp/.X*-lock
mkdir -p /tmp/.X11-unix || log_error "Failed to create /tmp/.X11-unix"
sudoIf chmod 1777 /tmp/.X11-unix || log_error "Failed to chmod /tmp/.X11-unix"
sudoIf chown root:"${group_name}" /tmp/.X11-unix || log_error "Failed to chown /tmp/.X11-unix"

# Parse resolution
if [ "$(echo "${VNC_RESOLUTION}" | tr -cd 'x' | wc -c)" = "1" ]; then
    VNC_RESOLUTION=${VNC_RESOLUTION}x16
fi
screen_geometry="${VNC_RESOLUTION%*x*}"
screen_depth="${VNC_RESOLUTION##*x}"

# Create xstartup script
# Variables are expanded from anyos.conf at runtime
log "Creating xstartup script..."
cat > /tmp/anyos-xstartup << XSTARTUP
#!/bin/bash
# Load X resources (Xft.dpi for font rendering)
[ -f ~/.Xresources ] && xrdb -merge ~/.Xresources

# GTK/Qt environment (from anyos.conf)
export GDK_SCALE=${GDK_SCALE}
export GDK_DPI_SCALE=${GDK_DPI_SCALE}
export QT_SCALE_FACTOR=${QT_SCALE_FACTOR}
export XCURSOR_SIZE=${XCURSOR_SIZE}
# Electron apps
export ELECTRON_FORCE_IS_PACKAGED=0
export ELECTRON_FORCE_DEVICE_SCALE_FACTOR=${GDK_SCALE}

# Software rendering for WebGL support in VNC (no hardware GPU)
export LIBGL_ALWAYS_SOFTWARE=1
export GALLIUM_DRIVER=llvmpipe

exec /usr/bin/startxfce4
XSTARTUP
chmod +x /tmp/anyos-xstartup || log_error "Failed to chmod xstartup script"

# === PHASE 3: Start Docker, VNC, and Plank config in parallel ===
log "Starting Docker, VNC, and configuring Plank in parallel..."

# Start Docker service in background
(
    docker_start_output=$(sudoIf service docker start 2>&1)
    docker_start_exit=$?
    echo "$docker_start_output" | tee -a /tmp/docker-service.log
    if [ $docker_start_exit -ne 0 ]; then
        echo "[$(date)] ERROR: Docker service start failed with exit code $docker_start_exit" | tee -a $LOG >&2
    fi
) &
docker_pid=$!

# Configure Plank dock in background (only needs D-Bus for dconf)
(
    log "Configuring Plank dock..."
    sudoUserIf dconf write /net/launchpad/plank/docks/dock1/dock-items "['google-chrome.dockitem', 'thunar.dockitem', 'xfce4-terminal.dockitem']" 2>/dev/null || true
    sudoUserIf dconf write /net/launchpad/plank/docks/dock1/theme "'WhiteSur-Light'" 2>/dev/null || true
    sudoUserIf dconf write /net/launchpad/plank/docks/dock1/hide-mode 0 2>/dev/null || true
    sudoUserIf dconf write /net/launchpad/plank/docks/dock1/icon-size "${ANYOS_DOCK_ICON_SIZE}" 2>/dev/null || true
    sudoUserIf dconf write /net/launchpad/plank/docks/dock1/pressure-reveal false 2>/dev/null || true
) &
plank_config_pid=$!

# Start VNC server (daemonizes itself)
log "Starting VNC server on display ${DISPLAY} at ${screen_geometry} ${VNC_DPI} DPI..."
vnc_output=$(sudoUserIf tigervncserver ${DISPLAY} \
    -geometry ${screen_geometry} \
    -depth ${screen_depth} \
    -rfbport ${VNC_PORT} \
    -dpi ${VNC_DPI} \
    -localhost \
    -desktop AnyOS \
    -SecurityTypes None \
    -xstartup /tmp/anyos-xstartup 2>&1)
vnc_exit=$?
if [ $vnc_exit -ne 0 ]; then
    log_error "VNC server start failed with exit code $vnc_exit"
    log_error "VNC output: $vnc_output"
    dump_vnc_debug
else
    log "VNC server started (exit code $vnc_exit)"
fi

# Wait for background tasks to complete
wait $docker_pid $plank_config_pid

# === PHASE 4: Wait for services to be ready ===

# Wait for X server to be accepting connections (timeout: 60s)
log "Waiting for X server to be ready..."
x_attempts=0
x_ready=false
while ! sudoUserIf xdpyinfo -display "${DISPLAY}" >/dev/null 2>&1; do
    sleep 0.5
    x_attempts=$((x_attempts + 1))
    if [ $((x_attempts % 20)) -eq 0 ]; then
        log "Still waiting for X server... (${x_attempts} attempts, $((x_attempts / 2))s)"
    fi
    if [ $x_attempts -ge 120 ]; then
        log_error "X server failed to become ready after 60 seconds"
        dump_x_debug
        break
    fi
done
if sudoUserIf xdpyinfo -display "${DISPLAY}" >/dev/null 2>&1; then
    log "X server is ready on display ${DISPLAY}"
    x_ready=true
else
    log_error "X server is NOT ready - continuing anyway"
fi

# Wait for Docker to be accessible (timeout: 60s)
log "Waiting for Docker to be accessible..."
docker_attempts=0
docker_ready=false
while ! docker info >/dev/null 2>&1; do
    # Try to fix socket permissions if socket exists
    if [ -S "/var/run/docker.sock" ]; then
        sudoIf chmod 666 /var/run/docker.sock 2>/dev/null || true
    fi
    sleep 0.5
    docker_attempts=$((docker_attempts + 1))
    if [ $((docker_attempts % 20)) -eq 0 ]; then
        log "Still waiting for Docker... (${docker_attempts} attempts, $((docker_attempts / 2))s)"
    fi
    if [ $docker_attempts -ge 120 ]; then
        log_error "Docker failed to become accessible after 60 seconds"
        dump_docker_debug
        break
    fi
done
if docker info >/dev/null 2>&1; then
    log "Docker is accessible"
    docker_ready=true
else
    log_error "Docker is NOT accessible - continuing anyway"
fi

# === PHASE 5: Post-X setup ===

# Start noVNC (web-based VNC access, needs VNC server running)
if [ -d "/usr/local/novnc" ]; then
    log "Starting noVNC on port ${NOVNC_PORT}..."
    /usr/local/novnc/noVNC-1.2.0/utils/launch.sh --listen ${NOVNC_PORT} --vnc localhost:${VNC_PORT} &
else
    log_error "noVNC directory not found at /usr/local/novnc"
fi

# Start Plank respawn loop (needs X and window manager to display the dock)
log "Starting Plank dock..."
(
    while true; do
        # Wait for X server before each start (handles X restarts)
        while ! sudoUserIf xdpyinfo -display "${DISPLAY}" >/dev/null 2>&1; do
            sleep 1
        done
        # Wait for window manager (xfwm4) to be ready
        if ! wait_for_window_manager; then
            log_error "Proceeding with Plank despite window manager detection failure"
        fi
        # Start Plank
        sudoUserIf plank 2>/dev/null
        log "Plank exited, restarting in 2 seconds..."
        sleep 2
    done
) &

# === ALL SETUP COMPLETE ===
log "AnyOS desktop initialization complete."
log "  - D-Bus: $(pgrep -x dbus-daemon > /dev/null && echo 'running' || echo 'NOT RUNNING')"
log "  - Docker: $([ "$docker_ready" = "true" ] && echo 'accessible' || echo 'NOT ACCESSIBLE')"
log "  - X server: $([ "$x_ready" = "true" ] && echo "ready on ${DISPLAY}" || echo 'NOT READY')"
log "  - noVNC: port ${NOVNC_PORT}"

# Run command or keep alive
if [ -n "$1" ]; then
    log "Executing \"$@\"."
    exec "$@"
else
    log "AnyOS desktop ready. Connect via noVNC on port ${NOVNC_PORT}."
    tail -f /dev/null
fi
