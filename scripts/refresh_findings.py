#!/usr/bin/env python3
"""Generate a compact latest-findings report from extracted artifacts."""

from __future__ import annotations

import datetime as dt
import json
import pathlib
import re
import subprocess
from collections import Counter


ROOT = pathlib.Path(__file__).resolve().parents[1]
EXTRACTED = ROOT / "extracted"
OUTPUT = ROOT / "LATEST_FINDINGS.md"


def read_text(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def read_json(path: pathlib.Path) -> dict:
    return json.loads(read_text(path))


def git_head() -> str:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=ROOT,
            capture_output=True,
            check=True,
            text=True,
        )
        return result.stdout.strip()
    except Exception:
        return "unknown"


def parse_rpc_summary(all_rpcs: list[str]) -> tuple[int, Counter[str]]:
    service_counts: Counter[str] = Counter()
    for rpc in all_rpcs:
        rpc = rpc.strip()
        if not rpc:
            continue
        service = ".".join(rpc.split(".")[:-1])
        service_counts[service] += 1
    return len([x for x in all_rpcs if x.strip()]), service_counts


def parse_dashboard_verb_summary(dashboard_methods: list[str]) -> Counter[str]:
    verbs: Counter[str] = Counter()
    for method in dashboard_methods:
        method = method.strip()
        if not method:
            continue
        match = re.match(r"([A-Z][a-z0-9]*)", method)
        verb = match.group(1) if match else "Other"
        verbs[verb] += 1
    return verbs


def classify_tags(tags: list[str]) -> Counter[str]:
    buckets: Counter[str] = Counter()
    for tag in tags:
        if tag.startswith("default-"):
            buckets["default"] += 1
        elif tag.startswith("browser-use-"):
            buckets["browser-use"] += 1
        elif tag.startswith("computer-use-"):
            buckets["computer-use"] += 1
        else:
            buckets["bare"] += 1
    return buckets


def parse_csv_block_after_header(enums_text: str, header_marker: str) -> list[str]:
    lines = enums_text.splitlines()
    for idx, line in enumerate(lines):
        if header_marker in line:
            block: list[str] = []
            for follow in lines[idx + 1 :]:
                stripped = follow.strip()
                if not stripped or stripped.startswith("## "):
                    break
                block.append(stripped)
            joined = " ".join(block)
            return [part.strip() for part in joined.split(",") if part.strip()]
    return []


def parse_exec_daemon_hash(s3_url: str) -> str:
    match = re.search(r"exec-daemon-x64-([a-f0-9]+)\.tar\.gz", s3_url)
    return match.group(1) if match else "unknown"


def read_text_optional(path: pathlib.Path) -> str:
    return read_text(path) if path.exists() else ""


def read_json_optional(path: pathlib.Path) -> dict:
    return read_json(path) if path.exists() else {}


def extract_build_id(text: str) -> str:
    match = re.search(r"BuildID:\s*([a-f0-9]+)", text)
    return match.group(1) if match else "unknown"


def extract_enum_values(text: str, prefix: str) -> list[str]:
    values: list[str] = []
    seen: set[str] = set()
    for item in re.findall(rf"{re.escape(prefix)}[A-Z0-9_]+", text):
        if item not in seen:
            values.append(item)
            seen.add(item)
    return values


def render() -> str:
    now = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    head = git_head()

    all_rpcs_lines = read_text(EXTRACTED / "all-grpc-rpcs.txt").splitlines()
    dashboard_methods = read_text(EXTRACTED / "dashboard-service-rpcs.txt").splitlines()
    cursorvm_domains = [x.strip() for x in read_text(EXTRACTED / "cursorvm-manager-domains.txt").splitlines() if x.strip()]
    enums_text = read_text(EXTRACTED / "protobuf-enums-and-schemas.txt")

    total_rpcs, rpc_services = parse_rpc_summary(all_rpcs_lines)
    dashboard_verbs = parse_dashboard_verb_summary(dashboard_methods)
    models = parse_csv_block_after_header(enums_text, "Model IDs Referenced")
    subagents = parse_csv_block_after_header(enums_text, "SubagentType")

    default_cfg = read_json(EXTRACTED / "ecr-default-config.json")
    browser_cfg = read_json(EXTRACTED / "ecr-browser-use-config.json")
    computer_cfg = read_json(EXTRACTED / "ecr-computer-use-config.json")
    tags_json = read_json(EXTRACTED / "ecr-tags.json")
    tags = tags_json.get("tags", [])
    tag_counts = classify_tags(tags)

    exec_pkg = read_json(EXTRACTED / "exec-daemon-meta" / "package.json")
    s3_url = read_text(EXTRACTED / "exec-daemon-meta" / "s3-download-url.txt").strip()
    exec_hash = parse_exec_daemon_hash(s3_url)

    # Newer extracted analysis artifacts (optional for backwards compatibility).
    cursorsandbox_analysis = read_text_optional(EXTRACTED / "binary-analysis" / "cursorsandbox-analysis.txt")
    pod_daemon_analysis = read_text_optional(EXTRACTED / "binary-analysis" / "pod-daemon-analysis.txt")
    isod_analysis = read_text_optional(EXTRACTED / "binary-analysis" / "isod-analysis.txt")
    polished_analysis = read_text_optional(EXTRACTED / "binary-analysis" / "polished-renderer-analysis.txt")
    dind_inspection = read_json_optional(EXTRACTED / "docker-in-docker-inspection.json")
    agent_modes_text = read_text_optional(EXTRACTED / "exec-daemon-code" / "agent-modes-and-models.txt")
    builtin_tools_text = read_text_optional(EXTRACTED / "exec-daemon-code" / "builtin-tools.txt")
    client_tools_text = read_text_optional(EXTRACTED / "exec-daemon-code" / "client-side-tools-v2.txt")
    s3_verdict = read_text_optional(EXTRACTED / "s3-access-verdict.txt").strip()
    s3_probe_results = read_text_optional(EXTRACTED / "s3-probe-results.txt").strip()
    skill_prompt_files = sorted((EXTRACTED / "exec-daemon-code").glob("skill-prompt-*_CONTENT.txt"))

    default_ports = sorted((default_cfg.get("config", {}).get("ExposedPorts") or {}).keys())
    browser_cmd = " ".join(browser_cfg.get("config", {}).get("Cmd") or [])
    computer_ports = sorted((computer_cfg.get("config", {}).get("ExposedPorts") or {}).keys())

    sandbox_has_7_step = "Sandbox Creation Pipeline (7 Steps)" in cursorsandbox_analysis
    sandbox_landlock = "Landlock" in cursorsandbox_analysis
    sandbox_seccomp = "Seccomp" in cursorsandbox_analysis
    sandbox_proxy = "HTTP proxy on 127.0.0.1" in cursorsandbox_analysis
    cursorsandbox_build = extract_build_id(cursorsandbox_analysis)
    pod_daemon_build = extract_build_id(pod_daemon_analysis)
    isod_build = extract_build_id(isod_analysis)
    polished_build = extract_build_id(polished_analysis)

    isod_rpc_total_match = re.search(r"RPCs \((\d+) total\)", isod_analysis)
    isod_rpc_total = isod_rpc_total_match.group(1) if isod_rpc_total_match else "unknown"
    isod_port_match = re.search(r"Port:\s*([0-9]+)", isod_analysis)
    isod_port = isod_port_match.group(1) if isod_port_match else "unknown"

    client_tools = extract_enum_values(client_tools_text, "CLIENT_SIDE_TOOL_V2_")
    builtin_tools = extract_enum_values(builtin_tools_text, "BUILTIN_TOOL_")
    agent_modes = extract_enum_values(agent_modes_text, "AGENT_MODE_")
    unified_modes = extract_enum_values(agent_modes_text, "UNIFIED_MODE_")
    thinking_styles = extract_enum_values(agent_modes_text, "THINKING_STYLE_")

    host_cfg = dind_inspection.get("host_config", {})
    docker_host = dind_inspection.get("docker_host", {})

    top_dashboard_verbs = dashboard_verbs.most_common(12)
    service_lines = "\n".join(
        f"- `{service}`: {count} RPCs" for service, count in sorted(rpc_services.items(), key=lambda kv: (-kv[1], kv[0]))
    )
    verb_lines = "\n".join(f"- `{verb}`: {count}" for verb, count in top_dashboard_verbs)
    model_line = ", ".join(f"`{m}`" for m in models) if models else "_none detected_"
    subagent_line = ", ".join(f"`{s}`" for s in subagents) if subagents else "_none detected_"
    domain_preview = ", ".join(f"`{d}`" for d in cursorvm_domains[:6])
    agent_mode_line = ", ".join(f"`{x}`" for x in agent_modes) if agent_modes else "_none detected_"
    unified_mode_line = ", ".join(f"`{x}`" for x in unified_modes) if unified_modes else "_none detected_"
    thinking_style_line = ", ".join(f"`{x}`" for x in thinking_styles) if thinking_styles else "_none detected_"

    evidence_files = [
        "extracted/all-grpc-rpcs.txt",
        "extracted/dashboard-service-rpcs.txt",
        "extracted/protobuf-enums-and-schemas.txt",
        "extracted/cursorvm-manager-domains.txt",
        "extracted/ecr-tags.json",
        "extracted/ecr-default-config.json",
        "extracted/ecr-browser-use-config.json",
        "extracted/ecr-computer-use-config.json",
        "extracted/exec-daemon-meta/package.json",
        "extracted/exec-daemon-meta/s3-download-url.txt",
    ]
    optional_evidence = [
        "extracted/binary-analysis/cursorsandbox-analysis.txt",
        "extracted/binary-analysis/pod-daemon-analysis.txt",
        "extracted/binary-analysis/isod-analysis.txt",
        "extracted/binary-analysis/polished-renderer-analysis.txt",
        "extracted/docker-in-docker-inspection.json",
        "extracted/exec-daemon-code/agent-modes-and-models.txt",
        "extracted/exec-daemon-code/builtin-tools.txt",
        "extracted/exec-daemon-code/client-side-tools-v2.txt",
    ]
    for rel in optional_evidence:
        if (ROOT / rel).exists():
            evidence_files.append(rel)
    if (EXTRACTED / "s3-access-verdict.txt").exists():
        evidence_files.append("extracted/s3-access-verdict.txt")
    if (EXTRACTED / "s3-probe-results.txt").exists():
        evidence_files.append("extracted/s3-probe-results.txt")
    for skill_prompt in skill_prompt_files:
        evidence_files.append(str(skill_prompt.relative_to(ROOT)))
    evidence_lines = "\n".join(f"- `{rel}`" for rel in evidence_files)

    s3_verdict_block = ""
    if s3_verdict:
        s3_verdict_block = f"""
## 8) S3 Access Verdict

{s3_verdict}
"""
    if s3_probe_results:
        s3_verdict_block += f"""
### Live Probe Evidence

{s3_probe_results}
"""

    skill_prompt_block = ""
    if skill_prompt_files:
        names = ", ".join(f"`{p.name}`" for p in skill_prompt_files)
        skill_prompt_block = f"""
## 9) Embedded Skill Prompt Templates

- Skill prompt files discovered: **{len(skill_prompt_files)}**
- Files: {names}
"""

    return f"""# Latest Findings Snapshot

_Auto-generated from extracted artifacts by `scripts/refresh_findings.py`._

- Generated: **{now}**
- Source commit: **`{head}`**

## 1) gRPC Surface (Current Dump)

- Total RPC methods discovered: **{total_rpcs}**
- DashboardService methods: **{len([m for m in dashboard_methods if m.strip()])}**

### Services
{service_lines}

### Dashboard verb distribution (top)
{verb_lines}

## 2) Runtime / Model Signal

- Models referenced in protobuf/enums: {model_line}
- Subagent types referenced: {subagent_line}
- Exec daemon build timestamp: **`{exec_pkg.get("buildTimestamp", "unknown")}`**
- Exec daemon S3 artifact hash: **`{exec_hash}`**

## 3) Image Variants + Tag Inventory

- ECR repo: **`{tags_json.get("name", "unknown")}`**
- Total tags observed: **{len(tags)}**
- Variant counts:
  - `default-*`: **{tag_counts.get("default", 0)}**
  - `browser-use-*`: **{tag_counts.get("browser-use", 0)}**
  - `computer-use-*`: **{tag_counts.get("computer-use", 0)}**
  - bare commit-like tags: **{tag_counts.get("bare", 0)}**

### Variant highlights
- `default-*` image created: **`{default_cfg.get("created", "unknown")}`**
- `default-*` exposed ports: **{", ".join(default_ports) if default_ports else "none"}**
- `browser-use-*` command: **`{browser_cmd or "unknown"}`**
- `computer-use-*` exposed ports: **{", ".join(computer_ports) if computer_ports else "none"}**

## 4) Control Plane Domain Footprint

- `cursorvm-manager.com` subdomains observed: **{len(cursorvm_domains)}**
- Sample: {domain_preview}

## 5) Sandbox + Daemon Internals

- `cursorsandbox` BuildID: **`{cursorsandbox_build}`**
- Sandbox pipeline includes 7-stage setup: **{str(sandbox_has_7_step).lower()}**
- Landlock present: **{str(sandbox_landlock).lower()}**, seccomp present: **{str(sandbox_seccomp).lower()}**
- Local HTTP proxy enforcement present: **{str(sandbox_proxy).lower()}**
- `pod-daemon` BuildID: **`{pod_daemon_build}`**
- `isod` BuildID / port / RPC count: **`{isod_build}`** / **{isod_port}** / **{isod_rpc_total}**
- `polished-renderer` BuildID: **`{polished_build}`**

## 6) Agent Mode + Tool Surface

- Agent modes: {agent_mode_line}
- Unified modes: {unified_mode_line}
- Thinking styles: {thinking_style_line}
- Client-side tool enum entries: **{len(client_tools)}**
- Builtin server tool enum entries: **{len(builtin_tools)}**

## 7) Docker-in-Docker Host Snapshot

- Host Docker version: **`{docker_host.get("docker_version", "unknown")}`**
- Host cgroup version: **`{docker_host.get("cgroup_version", "unknown")}`**
- Container host config:
  - `network_mode`: **`{host_cfg.get("network_mode", "unknown")}`**
  - `privileged`: **`{str(host_cfg.get("privileged", "unknown")).lower()}`**
  - `cpu_cores`: **`{host_cfg.get("cpu_cores", "unknown")}`**
  - `memory_gb`: **`{host_cfg.get("memory_gb", "unknown")}`**
{s3_verdict_block}
{skill_prompt_block}

## Evidence Files Used

{evidence_lines}
"""


def main() -> None:
    OUTPUT.write_text(render(), encoding="utf-8")
    print(f"Wrote {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
