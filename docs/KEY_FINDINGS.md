# Key Findings

High-confidence summary of the Cursor Background Agent architecture based on the extracted artifacts in this repository.

## Architecture at a Glance

- Workloads run in isolated cloud sandboxes on AWS.
- The sandbox environment includes Linux desktop tooling (XFCE + VNC + noVNC), Docker-in-Docker, and common language runtimes.
- Runtime control is split across multiple components: `pod-daemon`, `exec-daemon`, and `cursorsandbox`.

## Core Technical Findings

1. The public container image provides the environment; agent runtime binaries are injected at task runtime.
2. `pod-daemon` acts as container lifecycle/process manager and exposes a gRPC control surface.
3. `exec-daemon` orchestrates tool execution (shell, file ops, PTY, streaming protocol).
4. `cursorsandbox` enforces policy boundaries for command/file/network behavior.
5. The desktop stack is provisioned via Ansible (`extracted/ansible/vnc-desktop.yml`), not ad-hoc shell setup.
6. The sandbox includes Docker-in-Docker capabilities for running containerized user workloads.
7. Evidence files include protocol extracts for RPC/service surfaces and agent message schemas.
8. Tooling and system inventories show a preloaded multi-language development environment.
9. The architecture uses explicit control/data channels rather than a single monolithic process.
10. The repository supports claim tracing: docs summarize, `extracted/` provides underlying evidence.

## How to Validate Claims

- Read the full write-up in [ARCHITECTURE_REFERENCE.md](ARCHITECTURE_REFERENCE.md).
- Use [extracted/README.md](../extracted/README.md) to locate corresponding evidence files.
- Prefer direct file inspection when validating specific protocol, binary, or configuration claims.
