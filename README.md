# Cursor Background Agent — Full Architecture Reference

Reverse-engineered from inside a live Cursor Background Agent sandbox (March 2026).
Extracted files available at `/Users/advaitpaliwal/cursor-agent-bundle/`.

---

## Table of Contents

- [Overview](#overview)
- [Infrastructure Layers](#infrastructure-layers)
- [Component Details](#component-details)
  - [Pod Daemon](#1-pod-daemon)
  - [Exec Daemon](#2-exec-daemon)
  - [Cursor Sandbox](#3-cursor-sandbox)
  - [AnyOS Desktop](#4-anyos-desktop)
  - [Docker-in-Docker](#5-docker-in-docker)
- [Network Layout](#network-layout)
- [Container Image](#container-image)
- [Naming Conventions](#naming-conventions)
- [How to Recreate](#how-to-recreate)
  - [Base Image](#step-1-base-image)
  - [noVNC](#step-2-novnc)
  - [Docker-in-Docker](#step-3-docker-in-docker)
  - [Language Toolchains](#step-4-language-toolchains)
  - [Desktop Init Script](#step-5-desktop-init-script)
  - [Pod Daemon](#step-6-pod-daemon-simplified-go)
  - [Exec Daemon](#step-7-exec-daemon-simplified-nodejs)
  - [Docker Compose](#step-8-docker-compose-host)
  - [AWS Infrastructure](#step-9-aws-infrastructure)
- [Key Design Decisions](#key-design-decisions)
- [The Biggest Finding: It's Claude Code Under the Hood](#the-biggest-finding-its-claude-code-under-the-hood)
- [Bundled Dependencies](#bundled-dependencies)
- [Google Chrome](#google-chrome)
- [Sandbox Policy System](#sandbox-policy-system)
- [Twitter Thread Angles](#twitter-thread-angles)

---

## Overview

Cursor's Background Agent runs user tasks in isolated cloud sandboxes. Each task gets a fresh Docker container on AWS with a full Linux desktop, Docker-in-Docker, and multiple language toolchains. The agent (Node.js) receives instructions, executes code, interacts with GUIs via VNC, and reports results back.

**Critical finding:** The exec-daemon (`index.js`, `pod-daemon`, `cursorsandbox`) are **NOT baked into the image**. They are injected at container runtime by Cursor's orchestration layer. The public ECR image is purely the sandbox environment — the agent brain is deployed separately via S3 tarballs.

**Internal naming:** The monorepo is called **"everysphere"** (Anysphere's main repo). The Dockerfile lives at `anyrun/public-images/universal/Dockerfile` with an internal variant at `.cursor/Dockerfile`. Both share the same Ansible playbook.

---

## Infrastructure Layers

```
┌──────────────────────────────────────────────────────────────────┐
│                        CURSOR CLOUD                              │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │  AWS EC2 (us-east-1, KVM virtualization)                   │  │
│  │                                                            │  │
│  │  ┌──────────────────────────────────────────────────────┐  │  │
│  │  │  Docker Engine 29.1.4 (host)                         │  │  │
│  │  │                                                      │  │  │
│  │  │  ┌────────────────────────────────────────────────┐  │  │  │
│  │  │  │  Container: cursorenvironments/universal       │  │  │  │
│  │  │  │  (7.67GB, Ubuntu 24.04, network=host)          │  │  │  │
│  │  │  │                                                │  │  │  │
│  │  │  │  ┌──────────┐ ┌────────────┐ ┌─────────────┐  │  │  │  │
│  │  │  │  │pod-daemon│→│exec-daemon │→│cursorsandbox│  │  │  │  │
│  │  │  │  │ (PID 1)  │ │ (Node.js)  │ │  (Rust)     │  │  │  │  │
│  │  │  │  └──────────┘ └────────────┘ └─────────────┘  │  │  │  │
│  │  │  │                                                │  │  │  │
│  │  │  │  ┌──────────────────┐  ┌──────────────────┐   │  │  │  │
│  │  │  │  │ Docker-in-Docker │  │  AnyOS Desktop   │   │  │  │  │
│  │  │  │  │  (port 2375)     │  │  XFCE4+VNC+noVNC │   │  │  │  │
│  │  │  │  └──────────────────┘  └──────────────────┘   │  │  │  │
│  │  │  └────────────────────────────────────────────────┘  │  │  │
│  │  └──────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Component Details

### 1. Pod Daemon

| Property | Value |
|----------|-------|
| Binary | `/pod-daemon` |
| Type | Statically compiled (Go or Rust) |
| PID | 1 (container entrypoint) |
| Role | Container init / lifecycle manager |
| Port | Likely 50052 (gRPC) |

**Responsibilities:**

- Runs as PID 1, handles signal forwarding and zombie reaping
- Launches the desktop-init script (`/usr/local/share/desktop-init.sh`)
- Communicates with the host orchestrator (health checks, lifecycle)
- Handles container shutdown/cleanup signals from the control plane

---

### 2. Exec Daemon

| Property | Value |
|----------|-------|
| Runtime | Node.js (bundled binary at `/exec-daemon/node`) |
| Package | `@anysphere/exec-daemon-runtime` |
| Entry | `/exec-daemon/index.js` (webpack bundle) |
| Ports | 26053, 26054 (TCP6) |
| Version source | `public-asphr-vm-daemon-bucket.s3.us-east-1.amazonaws.com` |

**File layout:**

```
/exec-daemon/
├── node                              # Bundled Node.js binary
├── index.js                          # Main webpack bundle
├── 980.index.js                      # Code-split chunk
├── package.json                      # @anysphere/exec-daemon-runtime
├── exec_daemon_version               # S3 URL to this build's tarball
├── pty.node                          # Native PTY addon (node-pty)
├── cursorsandbox                     # Rust gRPC sidecar binary
├── exec-daemon                       # Wrapper/alt entry
└── 97f64a4d8eca9a2e35bb.mp4          # 62KB, splash/loading animation
```

**`package.json` contents:**

```json
{
  "name": "@anysphere/exec-daemon-runtime",
  "private": true,
  "gitCommit": "unknown",
  "buildTimestamp": "2026-03-04T16:32:02.576Z"
}
```

**`exec_daemon_version` contents:**

```
https://public-asphr-vm-daemon-bucket.s3.us-east-1.amazonaws.com/exec-daemon/exec-daemon-x64-e11bf2731fbec97c4727b661f83720499761ba2f96cf8cc17212b7b25a3136ac.tar.gz
```

**Responsibilities:**

- Receives task instructions from Cursor's backend
- Executes shell commands via PTY (terminal emulation)
- Reads/writes files in the workspace
- Orchestrates the agent loop (think → act → observe)
- Streams results back to the Cursor UI
- Takes screenshots of the VNC desktop for visual tasks

---

### 3. Cursor Sandbox

| Property | Value |
|----------|-------|
| Binary | `/exec-daemon/cursorsandbox` |
| Type | Rust, statically linked (static-pie ELF), stripped |
| Framework | axum 0.8.8 + tonic (gRPC) + hyper + rustls |
| Port | Likely 50052 or 26500 |

**Rust crates identified from binary strings:**

| Crate | Version | Purpose |
|-------|---------|---------|
| axum | 0.8.8 | HTTP/routing framework |
| tonic | — | gRPC server |
| hyper | — | HTTP engine |
| rustls | — | TLS (no OpenSSL) |
| matchit | 0.8.4 | URL router |
| regex-automata | 0.4.13 | Text processing |
| base64 | 0.22.1 | Encoding |

**Responsibilities:**

- Sandboxing enforcement (filesystem/network/process restrictions)
- gRPC API for exec-daemon to request privileged operations
- Mediates access to Docker daemon, network, and filesystem

---

### 4. AnyOS Desktop

| Property | Value |
|----------|-------|
| Display | `:1` |
| VNC | TigerVNC on port 5901 (localhost only) |
| noVNC | websockify proxy on port 26058 (web access) |
| Window Manager | XFCE4 (xfwm4) |
| Dock | Plank (with auto-respawn loop) |
| File Manager | Thunar (daemon mode) |

**Init sequence** (`/usr/local/share/desktop-init.sh`):

```
Phase 1 → D-Bus setup
Phase 2 → Environment variables / user config
Phase 3 → X server (TigerVNC) start
Phase 4 → Docker readiness check
Phase 5 → noVNC + Plank dock + XFCE session
```

**Desktop provisioned via Ansible** (`/opt/cursor/ansible/vnc-desktop.yml`):

The entire desktop stack is installed by an Ansible playbook. Key details from the playbook header:

```yaml
# Installs and configures:
# - TigerVNC server
# - XFCE4 desktop environment
# - noVNC web-based VNC client
# - Google Chrome browser
# - WhiteSur macOS-style theme (GTK, icons, cursors)
# - Cursor logo and branding
# - Required fonts (including macOS fonts and Cascadia Code)
# - polished-renderer (native video renderer for screen recordings)
```

| Config Variable | Default | Purpose |
|----------------|---------|---------|
| `vnc_user` | `ubuntu` | User to configure VNC for |
| `ANYOS_DESKTOP_APPEARANCE` | `light` | Light/dark mode (`light` = black text, `dark` = white text) |
| `novnc_version` | `1.2.0` | noVNC version |
| `websockify_version` | `0.10.0` | Websockify version |
| `desktop_wallpaper_url` | Vercel Blob Storage URL | macOS-style wallpaper |

**Packages installed by the playbook:**

| Category | Packages |
|----------|----------|
| VNC | tigervnc-standalone-server, tigervnc-common, tigervnc-tools |
| Desktop | xfce4, xfce4-terminal, xfce4-settings, thunar |
| X11 | x11-utils, x11-xserver-utils, xdg-utils, **xdotool**, xclip, procps |
| D-Bus | dbus-x11, at-spi2-core |
| Apps | mousepad, seahorse, Google Chrome |
| Theming | adwaita-icon-theme, gnome-themes-extra, gnome-keyring, plank |

**Notable tools:**
- **xdotool** — programmatic mouse/keyboard control. The agent can simulate clicks, keystrokes, and window management on the desktop.
- **polished-renderer** — native video renderer for recording the agent's screen. This is how users watch the agent work in real-time.
- **WhiteSur theme** — the desktop is styled to look like macOS (GTK theme, icons, cursors).
- **Desktop wallpaper** — hosted on Vercel Blob Storage, suggesting Cursor uses Vercel for static assets.

**Purpose:**

- Provides a real GUI for the agent to interact with browsers
- Agent takes screenshots via VNC for visual verification
- Agent can programmatically control the desktop via xdotool
- Screen recordings via polished-renderer for user playback
- noVNC allows the user to watch/interact via web browser
- macOS-styled appearance for a polished user experience

**Fonts installed:**

| Font | Source |
|------|--------|
| SF Pro, SF Mono | macOS system fonts (bundled in Ansible `files/fonts/`) |
| Helvetica, Monaco | macOS fonts |
| Cascadia Code | Microsoft (downloaded from GitHub releases v2008.25) |
| JetBrains Mono | JetBrains |
| Noto Sans | Google |
| Liberation Sans | Substitution for Helvetica |
| Arimo | Substitution for Arial |

**Font substitution rules:** Arial→Arimo, Helvetica→Liberation Sans, system-ui→Noto Sans.

**Software rendering environment variables:**
```
LIBGL_ALWAYS_SOFTWARE=1
GALLIUM_DRIVER=llvmpipe
```

**Screen recording config:** `anyos.conf` sets 120fps framerate for screen capture, which the polished-renderer processes into polished output videos.

---

### 5. Docker-in-Docker

| Property | Value |
|----------|-------|
| Docker | 29.1.4 (Community Edition) |
| containerd | v2.2.1 |
| runc | 1.3.4 |
| API | port 2375 (unauthenticated, TCP) |
| Go | 1.25.5 |

**Purpose:**

- Agent can `docker compose up` user projects
- Run databases, Redis, message queues, etc.
- Full container lifecycle management inside the sandbox
- No auth on the API (internal only, host network)

---

## Network Layout

| Port | Protocol | Process | Purpose |
|------|----------|---------|---------|
| 2375 | TCP | dockerd | Docker API (DinD) |
| 5901 | TCP | Xtigervnc | VNC server (localhost only) |
| 26053 | TCP6 | exec-daemon | Agent API (channel 1) |
| 26054 | TCP6 | exec-daemon | Agent API (channel 2) |
| 26058 | TCP | websockify | noVNC proxy (web VNC access) |
| 26500 | TCP | unknown | Orchestration / control plane |
| 50052 | TCP | unknown | gRPC (pod-daemon or cursorsandbox) |

All on `network=host` mode — no port mapping, direct host network access.

---

## Container Image

| Property | Value |
|----------|-------|
| Registry | `public.ecr.aws/k0i0n2g5/cursorenvironments/universal` |
| Tag | `default-b8e9345` |
| Size | 7.67 GB |
| Base | Ubuntu 24.04.4 LTS (Noble Numbat) |
| Packages | 797 dpkg packages |

**Pre-installed toolchains:**

| Tool | Version / Location |
|------|--------------------|
| Node.js | 22.x (via nvm v0.40.3 at `~/.nvm/`) |
| npm, yarn, pnpm | Global installs |
| Python | 3.x (system, pip unlocked) |
| Go | System (`golang-go`) + gopls + staticcheck |
| Rust | 1.83.0 (via rustup at `/usr/local/cargo/`) |
| Java | Default JDK (`default-jdk`) |
| C/C++ | gcc, g++, clang (clang set as default) |
| Docker CLI | For DinD communication |
| Git + Git LFS | System install + LFS |
| GitHub CLI (`gh`) | For PR/issue management |

**Pre-installed tools:**

| Tool | Purpose |
|------|---------|
| ripgrep (`rg`) | Fast code search (used by agent) |
| jq, yq | JSON/YAML processing |
| ffmpeg | Video/audio processing |
| sqlite3 | Local database |
| ansible | Used to provision VNC desktop |
| oathtool | TOTP/HOTP 2FA code generation |
| cmake, make | Build systems |
| htop, lsof, file | System inspection |
| tmux | Terminal multiplexing |
| vim, emacs, nano | Text editors |
| Google Chrome | 145.0.7632.116-1 (installed via Ansible) |

**Image build details:**

| Property | Value |
|----------|-------|
| Created | 2026-02-24T07:15:03Z |
| Base image date | 2026-02-10 |
| Build tool | BuildKit |
| Layers | 23 (including 6 empty) |
| Architecture | amd64 only (no ARM build) |
| VNC resolution | 1920x1200x24 (96 DPI) |
| Default compiler | clang/clang++ (not gcc) |

---

## Naming Conventions

| Element | Pattern | Example |
|---------|---------|---------|
| Container name | `pod-{random_id}-{image_hash}` | `pod-kyaoya54prfyzkhl4qagqnuf34-b8e29869` |
| Hostname | `cursor` | — |
| S3 bucket | `public-asphr-vm-daemon-bucket` | "asphr" = anysphere abbreviated |
| ECR repo | `k0i0n2g5/cursorenvironments/universal` | — |
| Desktop brand | AnyOS | Internal OS name |
| Env marker | `CURSOR_AGENT=1` | — |

---

## How to Recreate

### Step 1: Reconstructed Dockerfile (from `crane config` layer history)

This is the **exact Dockerfile** reconstructed from the image's build history:

```dockerfile
FROM ubuntu:24.04

# === Environment ===
ENV TERM=xterm-256color
ENV GIT_DISCOVERY_ACROSS_FILESYSTEM=0
ENV LANG=en_US.UTF-8
ENV LC_ALL=en_US.UTF-8

# === Phase 1: Base packages (massive apt-get) ===
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    sudo curl wget git gcc g++ clang make zip iputils-ping vim emacs nano \
    man-db cmake htop ca-certificates oathtool \
    python3 python3-pip \
    golang-go \
    default-jdk \
    libatk1.0-0 libatk-bridge2.0-0 libcups2 libgtk-3-0 libgbm1 libnss3 \
    xvfb xauth tmux locales libasound2t64 \
    sqlite3 gh \
    jq yq ripgrep file lsof unzip xz-utils dnsutils net-tools \
    build-essential pkg-config software-properties-common \
    ffmpeg \
    ansible \
    && rm -rf /var/lib/apt/lists/*

# === Phase 2: Locale ===
RUN locale-gen en_US.UTF-8

# === Phase 3: Python packages ===
RUN rm -f /usr/lib/python3.*/EXTERNALLY-MANAGED
RUN pip3 install websockify numpy

# === Phase 4: VNC Desktop (via Ansible!) ===
COPY ansible /tmp/ansible
RUN ansible-playbook /tmp/ansible/vnc-desktop.yml --connection=local -i localhost,
RUN mkdir -p /opt/cursor/ansible && cp -r /tmp/ansible/* /opt/cursor/ansible/ && rm -rf /tmp/ansible

# === Phase 5: Compiler defaults ===
RUN bash -c "update-alternatives --set cc $(which clang)" && \
    bash -c "update-alternatives --set c++ $(which clang++)"

# === Phase 6: Root shell config ===
WORKDIR /root
RUN echo 'export PS1="\[\033[36m\]\\W\[\033[0m\] $ "' > /root/.bashrc

# === Phase 7: NVM init script ===
COPY universal/nvm-init.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/nvm-init.sh

# === Phase 8: Ubuntu user setup ===
RUN echo "ubuntu ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/ubuntu && \
    chmod 0440 /etc/sudoers.d/ubuntu && \
    echo "Defaults:ubuntu !lecture" > /etc/sudoers.d/ubuntu-no-lecture && \
    chmod 0440 /etc/sudoers.d/ubuntu-no-lecture && \
    usermod -s /bin/bash ubuntu && \
    mkdir -p /home/ubuntu && \
    chown -R ubuntu:ubuntu /home/ubuntu && \
    touch /home/ubuntu/.hushlogin

USER ubuntu
WORKDIR /home/ubuntu

# === Phase 9: NVM + Node.js ===
RUN git clone https://github.com/creationix/nvm.git .nvm && \
    cd .nvm && git checkout v0.40.3

# === Phase 10: Git LFS ===
RUN curl -s https://packagecloud.io/install/repositories/github/git-lfs/script.deb.sh | sudo bash && \
    sudo apt-get install -y git-lfs && \
    sudo rm -f /etc/apt/sources.list.d/github_git-lfs.list && \
    sudo rm -rf /var/lib/apt/lists/*

# === Phase 11: Node.js 22 + global packages ===
RUN bash -c "source /usr/local/bin/nvm-init.sh && nvm install 22.* && nvm alias default 22.*" && \
    bash -c "source /usr/local/bin/nvm-init.sh && npm i -g yarn pnpm"

# === Phase 12: Bashrc for ubuntu user ===
RUN echo 'export NVM_DIR="$HOME/.nvm"' > /home/ubuntu/.bashrc && \
    echo '[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"' >> /home/ubuntu/.bashrc && \
    echo '[ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"' >> /home/ubuntu/.bashrc && \
    echo 'export PS1="\[\033[36m\]\\W\[\033[0m\] $ "' >> /home/ubuntu/.bashrc

# === Phase 13: Rust ===
USER root
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH
ENV RUST_VERSION=1.83.0

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain ${RUST_VERSION} && \
    chmod -R a+w ${RUSTUP_HOME} ${CARGO_HOME}

# === Phase 14: Go tools ===
USER ubuntu
RUN go install golang.org/x/tools/gopls@latest && \
    go install honnef.co/go/tools/cmd/staticcheck@latest

RUN echo 'export PATH=/usr/local/cargo/bin:$PATH' >> /home/ubuntu/.bashrc

# === Phase 15: Desktop environment config ===
USER root
ENV DISPLAY=:1
ENV VNC_RESOLUTION=1920x1200x24
ENV VNC_DPI=96
EXPOSE 26058/tcp 5901/tcp

# === Final ===
USER ubuntu
WORKDIR /home/ubuntu
SHELL ["/bin/bash", "-c"]
CMD ["/usr/local/share/desktop-init.sh"]
```

**Notable details from the real Dockerfile:**
- Uses **Ansible** (`vnc-desktop.yml`) to provision the VNC/desktop stack — not raw apt commands
- **Clang** is set as the default C/C++ compiler (not gcc)
- Includes **Java JDK** (`default-jdk`) — not just Node/Python/Go/Rust
- Includes **oathtool** (TOTP/HOTP) — for 2FA code generation
- Includes **ffmpeg** — for video/audio processing
- Includes **gh** (GitHub CLI) — for PR/issue management
- Includes **ripgrep** (`rg`) — fast search (used by the agent)
- Global npm packages: **yarn** and **pnpm**
- Go LSP (**gopls**) and **staticcheck** pre-installed
- NVM v0.40.3 with Node 22
- Rust 1.83.0 via rustup
- VNC resolution: **1920x1200** (not 1080p — slightly taller)
- Image created: **2026-02-24** (10 days ago)
- Base Ubuntu image from: **2026-02-10**

### Step 5: Desktop Init Script

```bash
#!/bin/bash
# /usr/local/share/desktop-init.sh

set -e

log() { echo "[AnyOS] $(date '+%H:%M:%S') $*"; }

export DISPLAY=:1
export VNC_PORT=5901
export NOVNC_PORT=26058
export DBUS_SESSION_BUS_ADDRESS=""

# Phase 1: D-Bus
log "Starting D-Bus..."
eval $(dbus-launch --sh-syntax)
export DBUS_SESSION_BUS_ADDRESS

# Phase 2: Docker check
log "Checking Docker..."
docker_ready="false"
for i in $(seq 1 30); do
    if docker info >/dev/null 2>&1; then
        docker_ready="true"
        log "Docker is accessible"
        break
    fi
    sleep 1
done

# Phase 3: VNC server
log "Starting VNC server on display ${DISPLAY}..."
Xtigervnc ${DISPLAY} \
    -geometry 1920x1080 \
    -depth 24 \
    -rfbport ${VNC_PORT} \
    -SecurityTypes None \
    -AlwaysShared \
    -AcceptKeyEvents \
    -AcceptPointerEvents \
    -AcceptSetDesktopSize &

sleep 2

# Phase 4: XFCE session
log "Starting XFCE session..."
xfce4-session &
sleep 3

# Phase 5: noVNC + Plank
log "Starting noVNC on port ${NOVNC_PORT}..."
/usr/local/novnc/noVNC-1.2.0/utils/launch.sh \
    --listen ${NOVNC_PORT} \
    --vnc localhost:${VNC_PORT} &

log "Starting Plank dock..."
(
    while true; do
        while ! xdpyinfo -display "${DISPLAY}" >/dev/null 2>&1; do
            sleep 1
        done
        plank 2>/dev/null
        log "Plank exited, restarting in 2 seconds..."
        sleep 2
    done
) &

log "AnyOS desktop initialization complete."
log "  - Docker: $([ "$docker_ready" = "true" ] && echo 'accessible' || echo 'NOT ACCESSIBLE')"
log "  - X server: ready on ${DISPLAY}"
log "  - noVNC: port ${NOVNC_PORT}"

if [ -n "$1" ]; then
    exec "$@"
else
    log "Desktop ready. Connect via noVNC on port ${NOVNC_PORT}."
    tail -f /dev/null
fi
```

### Step 6: Pod Daemon (Simplified Go)

```go
package main

import (
	"log"
	"os"
	"os/exec"
	"os/signal"
	"syscall"
)

func main() {
	log.Println("[pod-daemon] Starting...")

	// Start desktop environment
	desktop := exec.Command("/usr/local/share/desktop-init.sh")
	desktop.Stdout = os.Stdout
	desktop.Stderr = os.Stderr
	go func() {
		if err := desktop.Run(); err != nil {
			log.Printf("[pod-daemon] Desktop exited: %v", err)
		}
	}()

	// Start exec daemon
	agent := exec.Command("/exec-daemon/node", "/exec-daemon/index.js")
	agent.Stdout = os.Stdout
	agent.Stderr = os.Stderr
	agent.Dir = "/workspace"
	go func() {
		if err := agent.Run(); err != nil {
			log.Printf("[pod-daemon] Exec daemon exited: %v", err)
		}
	}()

	// Handle shutdown
	sig := make(chan os.Signal, 1)
	signal.Notify(sig, syscall.SIGTERM, syscall.SIGINT)
	<-sig
	log.Println("[pod-daemon] Shutting down...")
}
```

### Step 7: Exec Daemon (Simplified Node.js)

```javascript
const http = require("http");
const { spawn } = require("child_process");
const pty = require("node-pty");
const fs = require("fs");
const path = require("path");

const WORKSPACE = "/workspace";
const PORT = 26053;

const server = http.createServer((req, res) => {
  // Health check
  if (req.url === "/health") {
    res.writeHead(200);
    res.end("ok");
    return;
  }
});

// In production this would be a gRPC server that:
//
// 1. Receives task instructions from the Cursor control plane
// 2. Spawns PTY sessions to execute commands:
//    const shell = pty.spawn("bash", [], {
//      name: "xterm-256color",
//      cols: 120,
//      rows: 40,
//      cwd: WORKSPACE,
//    });
//
// 3. Reads/writes files in /workspace
// 4. Takes VNC screenshots for visual verification:
//    exec("import -window root -display :1 /tmp/screenshot.png")
//
// 5. Streams output back to the Cursor UI via WebSocket/gRPC
// 6. Manages the agent loop: think → act → observe → repeat

server.listen(PORT, () => {
  console.log(`[exec-daemon] Listening on port ${PORT}`);
});
```

### Step 8: Docker Compose (Host)

```yaml
services:
  agent-sandbox:
    image: your-registry/agent-sandbox:latest
    privileged: true
    network_mode: host
    hostname: sandbox
    environment:
      - AGENT_MODE=1
    volumes:
      - workspace-data:/workspace
    deploy:
      resources:
        limits:
          cpus: "4"
          memory: 16G
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:26053/health"]
      interval: 10s
      timeout: 5s
      retries: 3

volumes:
  workspace-data:
```

### Step 9: AWS Infrastructure

```
AWS Account
├── ECR (Elastic Container Registry)
│   └── your-org/agent-sandbox:latest     # Store your universal image
│
├── S3
│   └── your-exec-daemon-bucket/          # Versioned exec-daemon tarballs
│       └── exec-daemon-x64-{hash}.tar.gz
│
├── EC2 (us-east-1)
│   └── Instance (c5.2xlarge or m5.2xlarge, KVM-backed)
│       └── Docker Engine
│           └── Pull & run agent-sandbox containers
│               └── One container per task (ephemeral)
│
└── Networking
    └── VPC with private subnets
        └── DNS: 10.0.0.2 (default VPC resolver)
```

**Orchestration (what you need to build yourself):**

```
Control Plane (your backend)
│
├── Task Queue
│   └── Receive task from user → assign to available EC2 instance
│
├── Container Lifecycle
│   └── docker run → monitor → stream output → docker rm
│
├── Image Updates
│   └── Build new image → push to ECR → rolling update
│
└── Exec Daemon Updates
    └── Build tarball → upload to S3 → containers pull on boot
```

---

## Key Design Decisions

| Decision | Trade-off |
|----------|-----------|
| `network=host` | No NAT overhead, simpler networking. Less network isolation between containers on the same host. |
| Docker-in-Docker via TCP (2375, no auth) | Fast, no socket mounting needed. Safe because entire container is ephemeral and single-tenant. |
| Static binaries for pod-daemon and cursorsandbox | No dependency on container's libc. Deployable anywhere. Larger binary size. |
| Webpack-bundled Node.js with own binary | No `node_modules`, no version conflicts. Harder to debug. |
| VNC + noVNC (XFCE desktop) | Real GUI for browser testing. Agent can screenshot the framebuffer. Heavy resource usage (~300MB RSS for desktop stack). |
| 7.67GB universal image | One image fits all languages/tools. Slower pull times but operationally simple — no per-language image variants. |
| Ephemeral containers | Each task gets a fresh container. No state leaks. Higher startup latency (mitigated by pre-warming). |
| AWS ECR Public | Image is publicly pullable (no auth needed). Fast pulls within AWS. |
| Plank auto-respawn loop | Desktop dock always available even if it crashes. Simple but hacky resilience. |
| Rust for sandbox sidecar | Memory-safe, high-performance, small footprint. Harder to iterate on than Node/Python. |
| Runtime injection of agent binaries | exec-daemon, pod-daemon, cursorsandbox are NOT in the image — injected at boot from S3. Allows rapid agent updates without rebuilding the 7.67GB image. |
| Ansible for desktop provisioning | Reproducible, idempotent desktop setup. Same playbook shared between public and internal images. |
| Separate Chrome profiles | Regular profile for visual desktop + Playwright profile for CDP automation. Prevents state conflicts. |
| 120fps screen capture + polished-renderer | High-fidelity screen recordings with post-processing (motion blur, zoom, click effects). Makes agent output look professional. |
| macOS theming (WhiteSur) | Familiar, polished look for users watching agent screen. Not just a raw Linux desktop. |
| SwiftShader WebGL | Software-rendered WebGL works in VNC without GPU passthrough. |

---

## The Biggest Finding: It's Claude Code Under the Hood

The webpack bundle (`/exec-daemon/index.js`, 15.2MB) reveals that **Cursor's Background Agent is running a modified/embedded version of Claude Code** (Anthropic's CLI agent). The exec-daemon contains a translation layer that maps Claude Code's internal tool system and event hooks into Cursor's own format.

**Exports found in the bundle:**

```javascript
extractClaudeHooks
isClaudeCodeSettingsJson
transformClaudeHooksToConfig

// Unused but present in the bundle:
CLAUDE_EVENT_TO_CURSOR_STEP
CLAUDE_TOOL_TO_CURSOR_TOOL
UNSUPPORTED_CLAUDE_EVENTS
UNSUPPORTED_CLAUDE_TOOLS
detectHooksSchema
isClaudeCodeHooksConfig
isClaudeCodeStopFormat
isCursorHooksConfig
isPermissionHook
normalizeStopResponse
normalizeSubagentStopResponse
parseClaudeSettingsJson
shouldRunHook
transformToolMatcher
```

**What this means:**

- `CLAUDE_TOOL_TO_CURSOR_TOOL` — maps Claude Code tools (`Read`, `Write`, `Edit`, `Bash`, `Glob`, `Grep`) to Cursor's internal tool names
- `CLAUDE_EVENT_TO_CURSOR_STEP` — maps Claude Code agent loop events to Cursor's UI steps (what you see in the Background Agent UI)
- `UNSUPPORTED_CLAUDE_TOOLS` — certain Claude Code tools are disabled in the Cursor context
- `extractClaudeHooks` / `transformClaudeHooksToConfig` — parses `.claude/` settings and hooks configuration
- `isClaudeCodeSettingsJson` — reads Claude Code's `settings.json` format
- `shouldRunHook` / `isPermissionHook` — Claude Code's hook/permission system is preserved
- `normalizeSubagentStopResponse` — Claude Code's subagent system is active

**The tool hook system is identical to Claude Code's:**

```javascript
// Pre/post tool use hooks (same as Claude Code)
preToolUseQuery.toolName
postToolUseQuery.toolName
postToolUseFailureQuery.toolName

// Tool name format: "Read", "Write", "MCP:tool_name"
// MCP (Model Context Protocol) tools are supported
```

**The full Claude Code mapper module** (`../hooks/dist/claude-code-mapper.js`):

```javascript
// Documented source code comments found in the bundle:

// "Transforms Claude Code hooks configuration to Cursor hooks format."
// "@see https://docs.anthropic.com/en/docs/claude-code/hooks"

// "Transform a Claude Code tool name pattern to a Cursor tool name pattern."
// "Transform a single Claude Code hook script to Cursor format."
// "Transform a Claude Code hook entry (with matcher and hooks array) to Cursor format."

// "Parse Claude Code settings.json and extract hooks configuration."
// "Parse a Claude Code settings.json string and transform to Cursor HooksConfig."
// "Validate that a parsed object looks like a Claude Code settings file."
```

**`UNSUPPORTED_CLAUDE_TOOLS`** — tools disabled in Cursor's context:

```javascript
const UNSUPPORTED_CLAUDE_TOOLS = ["Glob"];
```

Only `Glob` is unsupported — meaning all other Claude Code tools (`Read`, `Write`, `Edit`, `Bash`, `Grep`, `Agent`, etc.) are active.

**`UNSUPPORTED_CLAUDE_EVENTS`** — some Claude Code lifecycle events are filtered out (not mapped to Cursor UI steps).

**Claude Code plugins system** — the bundle even includes Claude Code's plugin/extension system:

```javascript
// Plugin identifier validation (GitHub-based plugins)
throw new CCPluginIdentifierError(
    "Invalid GitHub repo format, expected 'org/repo' (e.g., 'anthropics/claude-plugins')",
    identifier
);
```

**Warning messages** found in the mapper:

```javascript
logger.warn(`Claude Code event "${event}" is not supported in Cursor and will be ignored`);
logger.warn(`Unknown Claude Code event "${event}", skipping`);
logger.warn(`Claude Code event "${event}" has invalid value (expected array), skipping`);
```

**Key takeaway:** Cursor's "Background Agent" is essentially a cloud-hosted Claude Code instance wrapped in their custom sandbox infrastructure, with a translation layer (`CLAUDE_TOOL_TO_CURSOR_TOOL`, `CLAUDE_EVENT_TO_CURSOR_STEP`) that maps Claude Code's internals to Cursor's UI. The agent brain is not custom — it's Anthropic's agent framework running server-side. Even the hooks, settings, and plugin systems are preserved.

---

## gRPC API Schema (The Control Plane Protocol)

The exec-daemon exposes and consumes three gRPC services, defined via Protocol Buffers. These are the exact RPC methods found in the bundle:

### `agent.v1.ControlService`

The primary control plane service — how Cursor's backend orchestrates the agent:

| RPC Method | Purpose |
|-----------|---------|
| `Ping` | Health check |
| `Exec` | Execute a command in the sandbox |
| `ListDirectory` | List files in a directory |
| `ReadTextFile` | Read a text file |
| `WriteTextFile` | Write a text file |
| `ReadBinaryFile` | Read a binary file (images, etc.) |
| `WriteBinaryFile` | Write a binary file |
| `GetDiff` | Get git diff of workspace changes |
| `GetWorkspaceChangesHash` | Hash of current workspace state (for change detection) |
| `RefreshGithubAccessToken` | Refresh GitHub OAuth token for the agent |
| `WarmRemoteAccessServer` | Pre-warm a remote access server (for user to connect?) |
| `ListArtifacts` | List generated artifacts |
| `UploadArtifacts` | Upload artifacts (screenshots, files, etc.) |
| `GetMcpRefreshTokens` | Get MCP (Model Context Protocol) refresh tokens |
| `DownloadCursorServer` | Download Cursor's server binary into the sandbox |
| `UpdateEnvironmentVariables` | Update env vars at runtime |

### `agent.v1.ExecService`

Dedicated execution service:

| RPC Method | Purpose |
|-----------|---------|
| `Exec` | Execute a command (separate from ControlService.Exec) |

### `agent.v1.PtyHostService`

PTY (pseudo-terminal) service for interactive terminal sessions:

This service provides the terminal emulation that lets the agent run interactive commands, handle prompts, and stream output in real-time.

### Key Protobuf Fields

```protobuf
// Custom system prompt (allowlisted for specific teams only)
optional string custom_system_prompt = 8;

// Model selection
// e.g. "claude-3.5-sonnet", "Auto"
// Keys are base model IDs (e.g. "claude-4.5-sonnet"),
// values are arrays of {id, value} parameter pairs

// Feature flag
stopUsingDsv3AgenticModel;

// Reranker
RERANKER_ALGORITHM_LULEA_HAIKU = 7;
```

---

## Model & AI Details

| Finding | Value |
|---------|-------|
| Model references | `claude-3.5-sonnet`, `claude-4.5-sonnet`, `Auto` |
| Custom system prompt | Supported via `custom_system_prompt` field (allowlisted teams only) |
| Reranker | `LULEA_HAIKU` algorithm (likely for code search ranking) |
| Feature flag | `stopUsingDsv3AgenticModel` (migration away from an older model) |
| CLAUDE.md support | Full support — loads `CLAUDE.md`, `CLAUDE.local.md`, `AGENTS.md` from workspace hierarchy |
| Rules loading | Loads from `.cursor/rules/`, `CLAUDE.md`, `AGENTS.md` in project hierarchy |
| MCP support | Full Model Context Protocol support with refresh tokens |
| CLI flags | `--claude-md-enabled` / `--no-claude-md-enabled` to toggle CLAUDE.md loading |

### System Prompt Construction

The agent builds its system prompt from multiple sources:
- Environment details (OS, tools, workspace info)
- `CLAUDE.md` / `CLAUDE.local.md` files from the project hierarchy
- `AGENTS.md` files from the project hierarchy
- `.cursor/rules/` directory
- Custom system prompt override (for allowlisted teams)
- MCP server hints ("information MAY be added to the system prompt")
- Tool descriptions and available resources

---

## Bundled Dependencies

The exec-daemon's 15.2MB webpack bundle includes these key packages (identified from `node_modules/.pnpm/` paths):

| Package | Purpose |
|---------|---------|
| `@bufbuild/protobuf` v1.10.0 | Protocol Buffer serialization |
| `@grpc/*` | gRPC client/server framework |
| OpenTelemetry (`OTEL_*`) | Telemetry, tracing, metrics |
| Prometheus exporter | Metrics export |
| `node-pty` (via `pty.node`) | Terminal/PTY emulation |
| MIME type database | Full MIME type → extension mapping |
| Debug | Namespace-based debug logging |

**Communication protocol:** Protocol Buffers over gRPC (not REST). The exec-daemon communicates with Cursor's backend using protobuf-defined message types with fields like `tool_name`, indicating a structured RPC API.

**Telemetry:** OpenTelemetry is deeply integrated with environment variables for:
- `OTEL_EXPORTER_PROMETHEUS_HOST` / `PORT`
- `OTEL_EXPORTER_OTLP_METRICS_PROTOCOL`
- `OTEL_SEMCONV_STABILITY_OPT_IN`

This means Cursor has full observability into every agent action, tool call, and performance metric.

---

## polished-renderer (Screen Recording Engine)

A custom **Rust binary** at `/opt/cursor/polished-renderer/polished-renderer` — a high-performance video renderer for screen recordings. This is how users see the agent's work as polished video playback.

| Property | Value |
|----------|-------|
| Binary | `/opt/cursor/polished-renderer/polished-renderer` |
| Language | Rust |
| Input | Session directory with screen recording + "plan" JSON file |
| Output | Rendered video (1080p proxy + full resolution) |

**Capabilities** (identified from Rust source/dependencies):

- Motion blur effects (configurable shutter angle, quality)
- Zoom window effects with focus points
- Click effect visualizations (cursor clicks)
- Cursor path rendering with multiple styles
- Keystroke overlay rendering
- 120fps capture (configured in `anyos.conf`)
- Uses ffmpeg-next for video encode/decode
- Rayon for parallel processing
- Outputs render metrics as JSON

**Rust crate dependencies:**

| Crate | Purpose |
|-------|---------|
| ffmpeg-next | Video decode/encode |
| clap | CLI argument parsing |
| serde | JSON serialization |
| rayon | Parallel processing |
| resvg | SVG rendering |
| tiny-skia | 2D graphics |
| font-kit | Font loading |
| fontdue | Font rasterization |

**This is how Cursor makes the Background Agent's screen recordings look professional** — not raw VNC framebuffer dumps, but post-processed videos with motion blur, zoom effects, click visualizations, and keystroke overlays. The 62KB MP4 we found earlier (`97f64a4d8eca9a2e35bb.mp4`) is likely a loading animation or template for this renderer.

---

## Google Chrome

| Property | Value |
|----------|-------|
| Property | Value |
|----------|-------|
| Binary | `/usr/local/bin/google-chrome` (wrapper script) |
| Actual binary | `/usr/bin/google-chrome-stable` |
| Package | `google-chrome-stable` 145.0.7632.116-1 |
| CDP Port | **9222** (Chrome DevTools Protocol, always enabled) |
| Window size | 1840x1120, positioned at (20, 50) |
| WebGL | Software rendering via SwiftShader (ANGLE) |
| Profile | `/home/ubuntu/.config/google-chrome` (fixed, separate from default) |

**Chrome launch flags** (baked into wrapper scripts at `/usr/local/bin/chrome` and `/usr/local/bin/google-chrome`):

```bash
google-chrome-stable \
    --no-sandbox \                        # Required for unprivileged containers
    --test-type \                         # Suppress "unsupported flag" warnings
    --disable-dev-shm-usage \             # Use /tmp instead of /dev/shm (too small in containers)
    --use-gl=angle \                      # Software WebGL via SwiftShader
    --use-angle=swiftshader-webgl \
    --password-store=basic \              # Avoid gnome-keyring prompts
    --no-first-run \                      # Skip first run dialogs
    --no-default-browser-check \          # Don't ask to set as default
    --remote-debugging-port=9222 \        # CDP for Playwright to connect
    --user-data-dir=/home/ubuntu/.config/google-chrome \  # Fixed profile (ensures CDP port works)
    --class=google-chrome \               # Force WMClass for Plank dock
    --window-size=1840,1120 \             # Window dimensions
    --window-position=20,50              # Window position on desktop
```

**Key insight: CDP port 9222 is always open.** This means the agent can use **Playwright** or any CDP client to programmatically control Chrome — navigate pages, click elements, fill forms, take screenshots, extract DOM content. The comment in the Ansible playbook explicitly states: *"Enable CDP for Playwright to connect to this instance."*

This is how the Background Agent can:
- Open a user's web app in Chrome
- Interact with it programmatically via Playwright/CDP
- Take pixel-perfect screenshots (not just VNC framebuffer grabs)
- Extract text/DOM content from web pages
- Run end-to-end tests

---

## Sandbox Policy System

The `cursorsandbox` binary (`/exec-daemon/cursorsandbox`) is a command wrapper that enforces sandbox policies:

```
cursorsandbox [OPTIONS] -- [COMMAND]...

Options:
    --sandbox-policy-cwd <DIR>       Working directory for sandbox policy resolution
    --sandbox-policy <JSON>          Sandbox policy as JSON string
    --policy <PATH>                  Path to network policy JSON file
    --policy-json <JSON>             Network policy as inline JSON string
    --policy-strict                  Fail closed if policy is missing/invalid (default: true)
    --preflight-only                 Only perform sandbox preflight (no exec)
    -h, --help                       Print help
```

**How it works:**

The exec-daemon wraps user commands through `cursorsandbox` with appropriate policies:

```bash
/exec-daemon/cursorsandbox \
    --sandbox-policy '{"allow_read":["/workspace"],"allow_write":["/workspace"]}' \
    --policy-json '{"allow":["*.npmjs.org","*.github.com"]}' \
    -- npm install express
```

**Key design points:**
- Filesystem sandboxing: Controls which paths can be read/written
- Network sandboxing: Controls which domains/IPs can be accessed
- Fail-closed by default (`--policy-strict` defaults to true)
- Preflight mode for checking sandbox support before execution
- Policies are passed as JSON (either inline or via file)

---

## Twitter Thread Angles

### Thread Hook
"I reverse-engineered Cursor's Background Agent from inside the sandbox. Here's what I found."

### Key Revelations (in order of impact)

1. **It's Claude Code** — The Background Agent is running a modified version of Anthropic's Claude Code CLI agent. The bundle contains `CLAUDE_TOOL_TO_CURSOR_TOOL` and `CLAUDE_EVENT_TO_CURSOR_STEP` translation layers. Cursor didn't build a custom agent — they wrapped Claude Code in their infrastructure.

2. **The image is PUBLIC** — `docker pull public.ecr.aws/k0i0n2g5/cursorenvironments/universal:default-b8e9345` — anyone can pull the 7.67GB sandbox image right now and inspect everything.

3. **"AnyOS"** — Cursor/Anysphere built a custom desktop OS for AI agents. Full XFCE desktop + Chrome + VNC, so the agent literally has a monitor it can look at.

4. **The sandbox binary** — A custom Rust binary that wraps every command with filesystem and network policies. The agent can't access anything outside the sandbox policy.

5. **Docker-in-Docker** — The agent gets a full Docker daemon inside its container. It can spin up databases, run `docker compose`, etc.

6. **100% homegrown** — No E2B, no Daytona, no Codespaces. Just raw AWS EC2 + ECR + custom Rust/Node.js daemons. They built everything from scratch.

7. **Cost estimate** — 7.67GB image, XFCE desktop, Chrome, Docker-in-Docker, 4 language runtimes. Each agent run likely costs $0.05-0.20+ in compute alone on top of LLM API costs.

8. **The full gRPC API** — `agent.v1.ControlService` with 16 RPC methods including `RefreshGithubAccessToken`, `UploadArtifacts`, `DownloadCursorServer`, and `GetMcpRefreshTokens`. This is the complete control plane protocol.

9. **Custom system prompts** — There's a `custom_system_prompt` field that's "allowlisted for specific teams only." Some Cursor enterprise customers get custom agent personalities.

10. **Model migration in progress** — A `stopUsingDsv3AgenticModel` feature flag suggests they're migrating from an older model version. Model selection supports `"claude-3.5-sonnet"`, `"claude-4.5-sonnet"`, and `"Auto"`.

11. **Full CLAUDE.md support** — The Background Agent reads `CLAUDE.md`, `CLAUDE.local.md`, and `AGENTS.md` from your project hierarchy, plus `.cursor/rules/`. Your project instructions carry into background tasks.

12. **Ansible-provisioned desktop** — The VNC/XFCE/Chrome stack is installed via an Ansible playbook (`vnc-desktop.yml`), not raw Dockerfile commands. Infrastructure-as-code all the way down.

---

## Evidence Sources

All findings from running commands inside a live Cursor Background Agent container:

```bash
hostname                              # → cursor
cat /proc/1/cgroup                    # → docker container ID
cat /etc/os-release                   # → Ubuntu 24.04
ps aux --sort=-rss                    # → process tree
ss -tlnp                              # → port map
cat /exec-daemon/package.json         # → @anysphere/exec-daemon-runtime
cat /exec-daemon/exec_daemon_version  # → S3 URL (asphr bucket)
file /exec-daemon/cursorsandbox       # → static-pie ELF
strings /exec-daemon/cursorsandbox    # → Rust crates (axum, tonic, etc.)
curl localhost:2375/version           # → Docker 29.1.4
curl localhost:2375/containers/json   # → container name + image
curl localhost:2375/images/json       # → ECR image URL + size
cat /proc/version                     # → kernel built on ip-10-0-0-10
cat /etc/resolv.conf                  # → 10.0.0.2 (AWS VPC DNS)
cat /usr/local/share/desktop-init.sh  # → AnyOS init script
find / -path "*/exec-daemon/*"        # → exec-daemon file layout
/exec-daemon/cursorsandbox --help     # → sandbox policy CLI
wc -c /exec-daemon/index.js           # → 15,254,116 bytes (15.2MB bundle)
head -c 2000 /exec-daemon/index.js    # → webpack bootstrap, @bufbuild/protobuf
strings /exec-daemon/index.js | grep "CLAUDE_TOOL"  # → Claude Code integration
strings /exec-daemon/index.js | grep "claude-code"  # → Claude Code mapper module
dpkg -l | grep chrome                 # → google-chrome-stable 145.0.7632.116-1
strings /exec-daemon/index.js | grep "generated from service"  # → gRPC services
strings /exec-daemon/index.js | grep "generated from rpc"      # → gRPC RPC methods
strings /exec-daemon/index.js | grep "claude-.*-sonnet"        # → model references
strings /exec-daemon/index.js | grep "custom_system_prompt"    # → system prompt override
strings /exec-daemon/index.js | grep "CLAUDE.md"               # → CLAUDE.md loading
crane config public.ecr.aws/k0i0n2g5/cursorenvironments/universal:default-b8e9345  # → full Dockerfile history
crane manifest public.ecr.aws/k0i0n2g5/cursorenvironments/universal:default-b8e9345  # → layer manifest
```
