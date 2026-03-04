# Latest Findings Snapshot

_Auto-generated from extracted artifacts by `scripts/refresh_findings.py`._

- Generated: **2026-03-04 23:09:37 UTC**
- Source commit: **`a7d7a0056d7b2d5a460ff62b3b4a6730b5b30b89`**

## 1) gRPC Surface (Current Dump)

- Total RPC methods discovered: **336**
- DashboardService methods: **313**

### Services
- `aiserver.v1.DashboardService`: 313 RPCs
- `agent.v1.ControlService`: 16 RPCs
- `agent.v1.PtyHostService`: 6 RPCs
- `agent.v1.ExecService`: 1 RPCs

### Dashboard verb distribution (top)
- `Get`: 113
- `Update`: 40
- `Create`: 19
- `Delete`: 17
- `List`: 17
- `Set`: 11
- `Add`: 6
- `Check`: 6
- `Revoke`: 6
- `Remove`: 4
- `Connect`: 3
- `Disconnect`: 3

## 2) Runtime / Model Signal

- Models referenced in protobuf/enums: `claude-3.5-sonnet`, `claude-4.5-sonnet`, `gpt-5`, `o1`, `o3`, `o4`, `codex`
- Subagent types referenced: `Bash`, `BrowserUse`, `ComputerUse`, `Config`, `CursorGuide`, `Custom`, `Debug`, `Explore`, `MediaReview`, `Shell`, `Unspecified`, `VmSetupHelper`
- Exec daemon build timestamp: **`2026-03-04T16:32:02.576Z`**
- Exec daemon S3 artifact hash: **`e11bf2731fbec97c4727b661f83720499761ba2f96cf8cc17212b7b25a3136ac`**

## 3) Image Variants + Tag Inventory

- ECR repo: **`k0i0n2g5/cursorenvironments/universal`**
- Total tags observed: **1000**
- Variant counts:
  - `default-*`: **268**
  - `browser-use-*`: **251**
  - `computer-use-*`: **51**
  - bare commit-like tags: **430**

### Variant highlights
- `default-*` image created: **`2026-02-24T07:15:03.526012958Z`**
- `default-*` exposed ports: **26058/tcp, 5901/tcp**
- `browser-use-*` command: **`bash`**
- `computer-use-*` exposed ports: **5900/tcp, 6080/tcp**

## 4) Control Plane Domain Footprint

- `cursorvm-manager.com` subdomains observed: **19**
- Sample: `dev.cursorvm-manager.com`, `eval1.cursorvm-manager.com`, `eval2.cursorvm-manager.com`, `test1.cursorvm-manager.com`, `train1.cursorvm-manager.com`, `train2.cursorvm-manager.com`

## 5) Sandbox + Daemon Internals

- `cursorsandbox` BuildID: **`e9c797b84b672ab7d3dd2d0982d5328100427f24`**
- Sandbox pipeline includes 7-stage setup: **true**
- Landlock present: **true**, seccomp present: **true**
- Local HTTP proxy enforcement present: **true**
- `pod-daemon` BuildID: **`83c327e927c0bd08a35e8d0dd0c6a0a5609ebfa3`**
- `isod` BuildID / port / RPC count: **`5e65ce955562cb2e5781e91f0a6c897c090d33c1`** / **50052** / **10**
- `polished-renderer` BuildID: **`e41b2334bad0d6721ef3dd003fcb55cfa6340aad`**

## 6) Agent Mode + Tool Surface

- Agent modes: `AGENT_MODE_UNSPECIFIED`, `AGENT_MODE_AGENT`, `AGENT_MODE_ASK`, `AGENT_MODE_PLAN`, `AGENT_MODE_DEBUG`, `AGENT_MODE_TRIAGE`, `AGENT_MODE_PROJECT`
- Unified modes: `UNIFIED_MODE_UNSPECIFIED`, `UNIFIED_MODE_CHAT`, `UNIFIED_MODE_AGENT`, `UNIFIED_MODE_EDIT`, `UNIFIED_MODE_CUSTOM`, `UNIFIED_MODE_PLAN`, `UNIFIED_MODE_DEBUG`
- Thinking styles: `THINKING_STYLE_UNSPECIFIED`, `THINKING_STYLE_DEFAULT`, `THINKING_STYLE_CODEX`, `THINKING_STYLE_GPT5`
- Client-side tool enum entries: **53**
- Builtin server tool enum entries: **20**

## 7) Docker-in-Docker Host Snapshot

- Host Docker version: **`29.1.4`**
- Host cgroup version: **`1`**
- Container host config:
  - `network_mode`: **`host`**
  - `privileged`: **`true`**
  - `cpu_cores`: **`4`**
  - `memory_gb`: **`16.0`**

## 8) S3 Access Verdict

S3 bucket probing verdict (public-asphr-vm-daemon-bucket):

- Unknown paths return uniform HTTP 403.
- This means you cannot distinguish "exists but denied" from "does not exist" by status code.
- Prefix/path brute-force enumeration is effectively blocked.
- Artifact access is content-addressable by exact hash in object path:
  exec-daemon/exec-daemon-x64-{sha256}.tar.gz
- Practical acquisition paths:
  1) Read /exec-daemon/exec_daemon_version from a running sandbox.
  2) Observe new sandbox sessions over time as deployments roll.

### Live Probe Evidence

S3 probe timestamp: 2026-03-04 23:09 UTC
Bucket: public-asphr-vm-daemon-bucket (us-east-1)

Known object probe:
- URL: /exec-daemon/exec-daemon-x64-e11bf2731fbec97c4727b661f83720499761ba2f96cf8cc17212b7b25a3136ac.tar.gz
- HTTP: 200
- Content-Length: 70431929
- Last-Modified: Wed, 04 Mar 2026 16:32:21 GMT
- SSE: AES256

Unknown object probes:
- URL: /exec-daemon/exec-daemon-x64-0000000000000000000000000000000000000000000000000000000000000000.tar.gz
  HTTP: 403
- URL: /exec-daemon/exec-daemon-x64-ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff.tar.gz
  HTTP: 403

Bucket root probe:
- URL: /
- HTTP: 403

Prefix probes:
- /exec-daemon/      -> 403
- /cursorsandbox/    -> 403
- /polished-renderer/-> 403
- /node/             -> 403
- /rg/               -> 403
- /gh/               -> 403
- /agent/            -> 403
- /pod-daemon/       -> 403


## 9) Embedded Skill Prompt Templates

- Skill prompt files discovered: **6**
- Files: `skill-prompt-CREATING_CURSOR_RULES_CONTENT.txt`, `skill-prompt-CREATING_SKILLS_CONTENT.txt`, `skill-prompt-CREATING_SUBAGENTS_CONTENT.txt`, `skill-prompt-MIGRATE_TO_SKILLS_CONTENT.txt`, `skill-prompt-SHELL_COMMAND_CONTENT.txt`, `skill-prompt-UPDATE_CURSOR_SETTINGS_CONTENT.txt`


## Evidence Files Used

- `extracted/all-grpc-rpcs.txt`
- `extracted/dashboard-service-rpcs.txt`
- `extracted/protobuf-enums-and-schemas.txt`
- `extracted/cursorvm-manager-domains.txt`
- `extracted/ecr-tags.json`
- `extracted/ecr-default-config.json`
- `extracted/ecr-browser-use-config.json`
- `extracted/ecr-computer-use-config.json`
- `extracted/exec-daemon-meta/package.json`
- `extracted/exec-daemon-meta/s3-download-url.txt`
- `extracted/binary-analysis/cursorsandbox-analysis.txt`
- `extracted/binary-analysis/pod-daemon-analysis.txt`
- `extracted/binary-analysis/isod-analysis.txt`
- `extracted/binary-analysis/polished-renderer-analysis.txt`
- `extracted/docker-in-docker-inspection.json`
- `extracted/exec-daemon-code/agent-modes-and-models.txt`
- `extracted/exec-daemon-code/builtin-tools.txt`
- `extracted/exec-daemon-code/client-side-tools-v2.txt`
- `extracted/s3-access-verdict.txt`
- `extracted/s3-probe-results.txt`
- `extracted/exec-daemon-code/skill-prompt-CREATING_CURSOR_RULES_CONTENT.txt`
- `extracted/exec-daemon-code/skill-prompt-CREATING_SKILLS_CONTENT.txt`
- `extracted/exec-daemon-code/skill-prompt-CREATING_SUBAGENTS_CONTENT.txt`
- `extracted/exec-daemon-code/skill-prompt-MIGRATE_TO_SKILLS_CONTENT.txt`
- `extracted/exec-daemon-code/skill-prompt-SHELL_COMMAND_CONTENT.txt`
- `extracted/exec-daemon-code/skill-prompt-UPDATE_CURSOR_SETTINGS_CONTENT.txt`
