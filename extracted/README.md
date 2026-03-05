# Extracted Artifacts Index

This folder contains raw extracted files and evidence used to build the architecture reference.
If you are new to the repo, read the [`Key Findings Snapshot`](../README.md#key-findings-snapshot) in the root `README.md` first and treat this directory as optional deep evidence.

## Directory Map

| Path | Contents |
| --- | --- |
| `ansible/` | VNC/XFCE provisioning playbook and packaged desktop assets |
| `binary-analysis/` | Reverse-engineering notes for key binaries (`pod-daemon`, `cursorsandbox`, renderer) |
| `computer-use-image/` | Process and service configs for computer-use image behavior |
| `config/` | Runtime configuration files (`anyos`, Docker daemon, display settings) |
| `cursor-logos/` | Cursor branding assets used in desktop setup |
| `exec-daemon-code/` | Extracted daemon internals (tooling, protocol, prompts, models, MCP) |
| `exec-daemon-meta/` | Packaged daemon metadata and launcher artifacts |
| `polished-renderer/` | Rust source snapshot for the recording/rendering engine |
| `scripts/` | Runtime bootstrap scripts (`start.sh`, `entrypoint.sh`, display setup) |
| `system/` | OS/package/env inventories and system strings |
| `xfce-config/` | Desktop environment configuration files |

## Top-Level Evidence Files

| File | Purpose |
| --- | --- |
| `all-grpc-rpcs.txt` | Enumerated RPC surface discovered from extracted services |
| `all-protobuf-enums.txt` | Aggregate protobuf enum extraction |
| `aiserver-api-catalog.txt` | API/service catalog notes |
| `credential-flow-analysis.txt` | Authentication and credential flow observations |
| `dashboard-service-rpcs.txt` | Dashboard/billing RPC extraction |
| `docker-in-docker-inspection.json` | DinD runtime inspection output |
| `network-topology.txt` | Live network and connectivity mapping |
| `pod-daemon-service.txt` | Pod daemon service and behavior notes |
| `protobuf-agent-v1-types.txt` | Agent protobuf type extraction |
| `protobuf-enums-and-schemas.txt` | Protobuf schema notes |

## Notes

- Some directories intentionally contain duplicated source snapshots captured from different extraction paths.
- Treat these files as evidence snapshots; prefer adding new files rather than overwriting historical data.
