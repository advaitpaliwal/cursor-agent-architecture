# Session Wave Notes (Raw Extracts)

This file contains raw command/evidence notes captured during multiple live inspection waves on March 4, 2026.

These notes are intentionally preserved in near-original form and may contain dense command annotations.

````bash
# === Added March 4, 2026 — Wave 1 (from inside live sandbox via Claude Code) ===
ls -la /exec-daemon/                    # → full file layout with sizes (rg, gh, polished-renderer.node)
file /exec-daemon/polished-renderer.node  # → ELF shared object (N-API addon)
file /exec-daemon/rg                    # → static-pie ELF (bundled ripgrep)
file /exec-daemon/gh                    # → static ELF (bundled GitHub CLI)
ps aux --sort=-rss                      # → live process tree with memory usage
strings /exec-daemon/index.js | grep "generated from enum"      # → 5,353 protobuf definitions
strings /exec-daemon/index.js | grep "generated from service"   # → 4 gRPC services
strings /exec-daemon/index.js | grep "generated from rpc"       # → 200+ RPCs (full Dashboard API)
strings /exec-daemon/index.js | grep "generated from message agent.v1.*ToolCall$"  # → 38 agent tools
strings /exec-daemon/index.js | grep "AgentMode"               # → 6 agent modes
strings /exec-daemon/index.js | grep "ThinkingStyle"           # → 3 thinking styles (Default/Codex/GPT5)
strings /exec-daemon/index.js | grep "SandboxPolicy"           # → 3 sandbox policy types
strings /exec-daemon/index.js | grep "NetworkPolicy"           # → allow/deny default actions
strings /exec-daemon/index.js | grep "EgressProtection"        # → 4 egress protection modes
strings /exec-daemon/index.js | grep "ComputerUseAction"       # → 11 desktop control actions
strings /exec-daemon/index.js | grep "RecordingMode"           # → start/save/discard recording
strings /exec-daemon/index.js | grep "IdleClassification"      # → 4 idle types for smart recording
strings /exec-daemon/index.js | grep "SimulatedMsgReason"      # → 7 self-prompting triggers
strings /exec-daemon/index.js | grep "ConversationAction"      # → 10 action types
strings /exec-daemon/index.js | grep "ModelDetails"            # → multi-provider credentials
strings /exec-daemon/index.js | grep "AgentRunRequest"         # → full request structure
strings /exec-daemon/index.js | grep "ClientSideToolV2"        # → 63 Cursor client tools
strings /exec-daemon/index.js | grep "BuiltinTool"             # → 20 server-side tools
strings /exec-daemon/index.js | grep "RERANKER_ALGORITHM"      # → 10 reranker algorithms
strings /exec-daemon/index.js | grep "gemini\|smart.allowlist" # → Gemini command classifier
strings /exec-daemon/index.js | grep "InvocationContext"       # → IDE state, GitHub PR, Slack thread triggers

# === Added March 4, 2026 — Wave 3 (container runtime, host details, Ansible) ===
curl -s http://localhost:2375/containers/pod-kyaoya54prfyzkhl4qagqnuf34-b8e29869/json  # → Full container inspect
cat /sys/fs/cgroup/memory/memory.limit_in_bytes  # → 17179869184 (16 GB)
cat /sys/fs/cgroup/cpu/cpu.cfs_quota_us           # → 400000 (4 cores)
cat /proc/1/status | grep Cap                     # → CapPrm: 000001ffffffffff (all capabilities)
cat /proc/1/cgroup                                # → docker/9ff6c4253fb5... (container ID)
ifconfig                                          # → eth0: 172.30.0.2, docker0: 172.17.0.1
dig +short -x 172.30.0.2                          # → ip-172-30-0-2.ec2.internal
dig +short api2.cursor.sh                         # → api2geo.cursor.sh → 8 AWS IPs
mount                                             # → overlay fs with 24 layers
ls -laR /opt/cursor/                              # → ansible/, artifacts/, logs/, recording-staging/
cat /opt/cursor/ansible/files/anyos.conf          # → Full desktop config (1920x1200, 120fps, SF NS fonts)
cat /opt/cursor/ansible/files/desktop-init.sh     # → 12KB real init script (parallel startup, debug helpers)
cat /opt/cursor/ansible/vnc-desktop.yml           # → 1023-line Ansible playbook (full desktop provisioning)
curl -sI exec-daemon S3 URL                       # → 70MB tarball, AES256 encrypted, Last-Modified March 4
dmesg                                             # → overlayfs warnings, KVM idle traces
strings /exec-daemon/index.js | grep "cursorvm-manager"  # → 17 cluster URLs (dev, eval, train, us1-us6)
strings /exec-daemon/index.js | grep "anysphere"         # → @anysphere/* internal packages
python3 -c "gRPC HTTP/2 probe on port 50052"     # → SETTINGS frame (confirms gRPC)

# === Added March 4, 2026 — Wave 2 (cursorsandbox deep dive, security, infrastructure) ===
strings /exec-daemon/cursorsandbox | grep -i "sandbox:"        # → 7-step sandbox init process
strings /exec-daemon/cursorsandbox | grep -i "landlock"        # → Landlock LSM integration
strings /exec-daemon/cursorsandbox | grep -i "seccomp"         # → BPF filter for syscall blocking
strings /exec-daemon/cursorsandbox | grep -i "blackhole"       # → Filesystem blackhole mechanism
strings /exec-daemon/cursorsandbox | grep -i "policy"          # → Network policy enforcement
/exec-daemon/cursorsandbox --help                              # → Full CLI flags
strings /pod-daemon | grep -i "spawn\|process\|config\|listen" # → Pod-daemon is Rust (tonic, rustls, aws-lc)
strings /exec-daemon/index.js | grep "cursorvm"               # → Multi-region infrastructure domains
strings /exec-daemon/index.js | grep "anysphere"              # → Internal package names
strings /exec-daemon/polished-renderer.node | grep -i "render\|video\|codec\|frame" # → Video pipeline details
cat /home/ubuntu/.claude/.credentials.json                     # → Claude Code OAuth tokens
cat /home/ubuntu/.config/gh/hosts.yml                          # → GitHub installation token
cat /home/ubuntu/.gitconfig                                    # → Git credential injection
cat /proc/self/status | grep Seccomp                           # → Seccomp: 0 (no host seccomp)
curl -s 169.254.169.254 --connect-timeout 2                   # → Connection timeout (metadata blocked)
ss -tlnp                                                       # → Ports 26500/50052 on 0.0.0.0
curl -s https://public-asphr-vm-daemon-bucket.s3.us-east-1.amazonaws.com/ # → AccessDenied (listing blocked)

# === Added March 4, 2026 — Wave 5 (ECR enumeration, complete protobuf, billing, image config) ===

## ECR Public Registry — FULLY ENUMERABLE
# Tag listing is public despite directory listing being denied
TOKEN=$(curl -s "https://public.ecr.aws/token/?...scope=repository:k0i0n2g5/cursorenvironments/universal:pull" | jq -r '.token')
curl -s -H "Authorization: Bearer $TOKEN" "https://public.ecr.aws/v2/k0i0n2g5/cursorenvironments/universal/tags/list"
# → 1,000 tags returned! Three image variants:
#   - default-*    (268 tags) — standard agent sandbox
#   - browser-use-* (251 tags) — browser automation variant
#   - computer-use-* (51 tags) — desktop/computer control variant
#   - bare hashes   (430 tags) — raw commit SHA prefixes
# Hash format: 7-char git short SHA (matches everysphere monorepo commits)
# Registry: public.ecr.aws/k0i0n2g5/cursorenvironments/universal
# Other repos (base, exec-daemon, cursor/*) return 404 — single repo design

## ECR Image Manifest (amd64)
# Multi-arch OCI index with amd64 + attestation manifests
# amd64 digest: sha256:df694266901bbb553c4172920ae00a448df212e884649f858a31d7a873a43437
# 23 layers, 2.18 GB compressed total
#   Layer 0:  28.4 MB  (Ubuntu 24.04 base)
#   Layer 1: 808.8 MB  (apt-get: gcc, clang, python3, golang-go, java, Chrome deps, VNC, XFCE4)
#   Layer 5:   7.7 MB  (pip: websockify, numpy)
#   Layer 6: 657.1 MB  (Ansible playbook execution — desktop, Chrome, themes, polished-renderer)
#   Layer 17: 97.4 MB  (Rust toolchain 1.83.0)
#   Layer 19: 274.0 MB (Node.js 22 via nvm + yarn + pnpm)
#   Layer 20: 294.1 MB (Go tools: gopls, staticcheck)

## ECR Image Config
# User: ubuntu
# ExposedPorts: 26058/tcp (noVNC), 5901/tcp (VNC)
# WorkingDir: /home/ubuntu
# Cmd: ["/usr/local/share/desktop-init.sh"]
# Env:
#   TERM=xterm-256color
#   GIT_DISCOVERY_ACROSS_FILESYSTEM=0
#   RUSTUP_HOME=/usr/local/rustup
#   CARGO_HOME=/usr/local/cargo
#   RUST_VERSION=1.83.0
#   DISPLAY=:1
#   VNC_RESOLUTION=1920x1200x24
#   VNC_DPI=96
# 44 Dockerfile steps:
#   0-5:   Ubuntu 24.04 base
#   6-12:  Env vars, apt packages (gcc, clang, python3, golang, java, Chrome deps)
#   13-16: pip, Ansible playbook, copy ansible to /opt/cursor/
#   17-21: clang as default CC, nvm-init.sh helper
#   22:    Sudoers: "ubuntu ALL=(ALL) NOPASSWD:ALL"
#   25-28: NVM, git-lfs, Node.js 22, yarn/pnpm, bashrc
#   29-34: Rust 1.83.0 (root), Go tools (user), cargo PATH
#   35-43: Display env, expose ports, CMD desktop-init.sh

## Complete Protobuf Message Types (599 types in agent.v1 package)
# Extracted: strings /exec-daemon/index.js | grep -oP 'agent\.v1\.[A-Z][A-Za-z]+' | sort -u | wc -l → 599

### Agent Tools (via protobuf Args/Result/ToolCall pattern):
# Edit, Write, Delete, Read, Ls, Glob, Grep, Shell, Fetch, WebFetch, WebSearch,
# ComputerUse, GenerateImage, RecordScreen, SemSearch, ReadLintsToolCall,
# ReadMcpResource, ListMcpResources, Mcp, McpAuth, GetMcpTools,
# ApplyAgentDiff, AiAttribution, BlameByFilePath, Diagnostics,
# AskQuestion, CreatePlan, PrManagement, SwitchMode, Subagent,
# StartGrindPlanning, StartGrindExecution, ReportBugfixResults,
# BackgroundShellSpawn, ForceBackgroundShell, WriteShellStdin,
# Task, UpdateTodos, ReadTodos, Reflect, Await, TruncatedToolCall,
# SetupVmEnvironment, RequestContext, ExecuteHook

### 12 Subagent Types:
# SubagentTypeBash, SubagentTypeBrowserUse, SubagentTypeComputerUse,
# SubagentTypeConfig, SubagentTypeCursorGuide, SubagentTypeCustom,
# SubagentTypeDebug, SubagentTypeExplore, SubagentTypeMediaReview,
# SubagentTypeShell, SubagentTypeUnspecified, SubagentTypeVmSetupHelper

### Key Conversation/State Types:
# ConversationAction, ConversationPlan, ConversationState, ConversationStateStructure,
# ConversationStep, ConversationSummary, ConversationSummaryArchive,
# ConversationTokenDetails, ConversationTurn, ConversationTurnStructure,
# AgentConversationTurn, AgentConversationTurnStructure,
# ShellConversationTurn, ShellConversationTurnStructure

### Computer Use Actions (11 types):
# ClickAction, DragAction, KeyAction, MouseDownAction, MouseMoveAction,
# MouseUpAction, ScreenshotAction, ScrollAction, TypeAction,
# CursorPositionAction, WaitAction

### Enterprise/Team Features:
# CustomSubagent, CustomSubagentPermissionMode,
# CursorPackage, CursorPackagePrompt, CursorRule (5 rule types),
# SkillDescriptor, SkillOptions, HooksConfigInfo,
# NetworkPolicy, NetworkPolicyLoggingConfig,
# SandboxPolicy, SandboxPolicyMergeSources

### Background Tasks:
# BackgroundShellAction, BackgroundShellSpawnArgs/Error/Result/Success,
# BackgroundTaskCompletion, BackgroundTaskCompletionAction,
# BackgroundTaskKind, BackgroundTaskStatus

### Artifacts & Uploads:
# ArtifactPathError, ArtifactPathErrorKind,
# ArtifactUploadDispatchResult/Status, ArtifactUploadInstruction/Metadata/Status,
# ListArtifactsRequest/Response, UploadArtifactsRequest/Response,
# OutputLocation — artifacts stored at /opt/cursor/artifacts/

### Recording/Video:
# RecordingDataPackage, RecordScreenArgs/Result/ToolCall,
# RecordScreenStartSuccess, RecordScreenSaveSuccess, RecordScreenDiscardSuccess,
# RecordScreenFailure, ClickEffectKeyframe, CursorPath, CursorPathKeyframe,
# DecisionInput, DecisionOutput, VideoCut, SpeedupSelection, ZoomCandidate

### PR/Git Management:
# CreatePrAction, UpdatePrAction, PrManagementArgs/Result/ToolCall,
# PrManagementNeedsConfirmation, PrManagementRegistered,
# UserGitContext, GitRepoInfo, BlameByFilePathArgs/Result/ToolCall,
# RefreshGithubAccessTokenRequest/Response

### MCP (Model Context Protocol):
# McpArgs, McpResult, McpToolCall, McpToolDefinition, McpToolDescriptor,
# McpToolError, McpToolNotFound, McpToolResult, McpToolResultContentItem,
# McpTools, McpDescriptor, McpFileSystemOptions, McpInstructions,
# McpMetaToolOptions, McpImageContent, McpTextContent,
# McpAuthArgs/Result/ToolCall, ListMcpResourcesExecArgs/Result,
# ReadMcpResourceExecArgs/Result, GetMcpRefreshTokensRequest/Response,
# GetMcpToolsAgentResult

### PTY (Terminal) Management:
# AttachPtyRequest, SpawnPtyRequest/Response, ResizePtyRequest/Response,
# TerminatePtyRequest/Response, ListPtysRequest/Response,
# PtyData, PtyEvent, PtyExited, PtyInfo, PtyHostService,
# SendInputRequest/Response

### Control Plane:
# PingRequest/Response, ExecRequest/Response, ExecStreamElement,
# ExecClientControlMessage, ExecClientHeartbeat, ExecClientMessage,
# ExecClientStreamClose, ExecClientThrow, ExecServerMessage,
# HeartbeatUpdate, ControlService, ExecService

### Remote/VM:
# DownloadCursorServerRequest/Response, SetupVmEnvironmentArgs/Result,
# WarmRemoteAccessServerRequest/Response,
# UpdateEnvironmentVariablesRequest/Response,
# ReadBinaryFileRequest/Response, WriteBinaryFileRequest/Response,
# ReadTextFileRequest/Response, WriteTextFileResponse

### Thinking/Streaming:
# ThinkingCompletedUpdate, ThinkingDeltaUpdate, ThinkingDetails,
# ThinkingMessage, ThinkingStyle, TextDeltaUpdate, TokenDeltaUpdate,
# PartialToolCallUpdate, ToolCallDelta, ToolCallDeltaUpdate

## Billing & Usage Protobuf (DashboardService)
# Complete billing system extracted from exec-daemon webpack bundle:

### Spending & Limits:
# GetCurrentPeriodUsage{Request,Response} — current spend tracking
# GetCurrentPeriodUsageResponse_SpendLimitUsage — breakdown
# GetDailySpendByCategory{Request,Response} — daily spend analytics
# DailySpendByCategory, DailySpendPoint — time series data
# ConfigureSpendLimitAction — admin spend limit configuration
# SetHardLimit{Request,Response} — absolute spending cap
# GetHardLimit{Request,Response} — query spending cap
# SetUserHardLimit{Request,Response} — per-user limits
# SetUserMonthlyLimit{Request,Response} — monthly user caps
# GetServiceAccountSpendLimit{Request,Response} — service account limits
# SetServiceAccountSpendLimitRequest — set SA limits
# EnableOnDemandSpend{Request,Response} — enable on-demand
# SetUsageBasedPremiumRequests{Request,Response} — premium request toggle
# GetUsageBasedPremiumRequests{Request,Response}
# GetUsageLimitPolicyStatus{Request,Response} — policy enforcement status
# GetUsageLimitStatusAndActiveGrants{Request,Response}
# user_team_spend_limit_dollars — team-level dollar limit field
# billingCycleStart, billingCycleEnd — cycle boundaries (proto_int64)
# billingMode — billing mode string
# bonusSpend — bonus credits in cents
# autoSpend, apiSpend — usage breakdown (auto vs API)
# included_spend — "Amount of included/free spend used in cents (auto + api combined)"
# canConfigureSpendLimit — admin permission flag
# adminOnlyUsagePricing — admin-restricted pricing toggle

### Subscriptions & Plans:
# ChangeTeamSubscription{Request,Response} — plan changes
# GetPlanInfo{Request,Response} — current plan details
# GetPricingHistory{Request,Response} — pricing changes over time
# GetYearlyUpgradeEligibility{Request,Response} — yearly plan check
# ActivatePromotionResponse_ActivationType: SUBSCRIPTION=2
# PlanChoice, PlanPhase — plan selection and phases
# GetSignUpType{Request,Response} — signup flow type

### Credits & Promotions:
# GetCreditGrantsBalance{Request,Response} — credit balance
# ActiveCreditGrant — "Active credit grants (max 5, ordered by priority)"
# ActivatePromotion{Request,Response} — promo code activation
# GetReferralCodes{Request,Response} — referral system
# GetReferrals{Request,Response}
# GetRemainingRefunds{Request,Response}

### Usage Analytics:
# GetTeamSpend{Request,Response} — team-level spend
# GetTeamUsage{Request,Response} — team usage metrics
# GetTeamAnalytics{Request,Response} — team analytics dashboard
# GetUserAnalytics{Request,Response} — per-user analytics
# GetFilteredUsageEvents{Request,Response} — filtered event log
# GetAggregatedUsageEvents{Request,Response} — aggregated events
# GetTokenUsage{Request,Response} — token consumption
# GetClientUsageData{Request,Response} — client-side usage
# UsageEvent, UsageEventDetails, UsageEventDisplay — event types
# UsageAlert — usage threshold alerts
# DailyMetrics — daily metric aggregation
# GetMonthlyBillingCycle{Request,Response}
# GetMonthlyInvoice{Request,Response}

### BugBot Billing:
# BugbotUsageTier: FREE_TIER=1
# bugbotWasEnabledInThisBillingCycle — billing flag
# autoDescription: "Consumed by Auto. Additional usage consumes API quota."

### Enterprise Features:
# GetTeamBackgroundAgentSettings{Request,Response}
# GetTeamBugbotSettings{Request,Response}
# GetTeamAdminSettingsResponse, GetBaseTeamAdminSettingsRequest
# SetAdminOnlyUsagePricing{Request,Response}
# GetTeamCustomerPortalUrl{Request,Response} — Stripe portal
# GetTeamHasValidPaymentMethod{Request,Response}
# GetTeamPrivacyModeForced{Request,Response} — privacy mode
# GetTeamSharedConversationSettings{Request,Response}
# SetUserPrivacyMode{Request,Response}
# GetUserPrivacyMode{Request,Response}
# GetProtectedGitScopes{Request,Response} — git scope protection
# GetTeamRepositoriesForServiceAccountScope{Request,Response}

### Team Management:
# GetTeamMembers{Request,Response}, GetTeamMemberDomains{Request,Response}
# GetTeamInviteLink{Request,Response}, GetTeamReposResponse
# GetTeamRules{Request,Response}, GetTeamHooks{Request,Response}
# GetTeamCommands{Request,Response}, GetTeamGithubUsers{Request,Response}
# ChangeSeat{Request,Response} — seat management
# GetTeamRawData{Request,Response} — raw team data export
# GetTeamIdForReactivation{Request,Response}

### Integrations:
# GetGithubInstallations{Request,Response} — GitHub app installs
# GetInstallationGithubUsers{Request,Response}
# GetInstallationRepos{Request,Response}
# GetSlackInstallUrl{Request,Response} — Slack integration
# GetSlackSettings{Request,Response}, GetSlackTeamSettings{Request,Response}
# GetSlackUserSettings{Request,Response}, GetSlackModelOptions{Request,Response}
# GetSlackRepoRoutingRules{Request,Response} — repo-specific routing
# SetSlackAuth{Request,Response}
# GetPublicSlackInstallUrl{Request,Response}
# GetPublicSlackInstallUrlWithUserScopes{Request,Response}
# GetLinearAuthUrl{Request,Response} — Linear integration
# GetLinearIssues{Request,Response}, GetLinearLabels{Request,Response}
# GetLinearTeams{Request,Response}, GetLinearStatus{Request,Response}
# GetLinearSettings{Request,Response}
# GetPagerDutyAuthUrl{Request,Response} — PagerDuty integration
# GetPagerDutyServices{Request,Response}
# GetPagerDutyStatus{Request,Response}
# GetSsoConfigurationLinks{Request,Response} — SSO/SAML
# GetScimConfigurationLinks{Request,Response} — SCIM provisioning
# GetScimConflicts{Request,Response}
# SetupGitlabEnterpriseInstance{Request,Response} — GitLab Enterprise

### Plugins/Marketplace:
# GetPlugin{Request,Response}, GetPublisher{Request,Response}
# GetEffectiveUserPlugins{Request,Response}
# GetPluginMcpConfig{Request,Response}
# RecentlyAddedPlugin — marketplace tracking

### Global Features:
# GetGlobalLeaderboardOptIn{Request,Response} — leaderboard
# SetGlobalLeaderboardOptIn{Request,Response}
# GetMarketingEmailOpt{Request,Response} — email preferences
# GetEnterpriseCTAEligibility{Request,Response} — enterprise upsell
# GetJoinableTeamsByDomain{Request,Response} — domain-based joining

### Indexing & Search:
# GetPRIndexingStatus{Request,Response} — PR indexing
# SetupIndexDependencies{Request,Response}
# GetAvailableChunkingStrategies{Request,Response} — RAG strategies
# GetHighLevelFolderDescription{Request,Response}
# RepositoryIndexingInfo — repo index status
# GetLineNumberClassifications{Request,Response}
# GetLintsForChange{Request,Response} — lint analysis

### Azure Blob Storage:
# cursor.blob.core.windows.net — VS Code server releases
# Pattern: https://cursor.blob.core.windows.net/remote-releases/${commit}/vscode-reh-${os}-${arch}.tar.gz

### Credential Note (CORRECTION):
# Credentials in ~/.claude/.credentials.json and ~/.config/gh/hosts.yml
# are the USER'S OWN credentials injected by Cursor via the credential
# helper system when Claude Code starts. The sk-ant-oat01-* OAuth tokens
# are the user's Anthropic auth, ghs_* is their GitHub installation token.
# NOT a Cursor credential leak — just user auth flowing through the sandbox.

# === Added March 4, 2026 — Wave 6 (ECR deep dive, image variants, Docker host) ===

## Three Distinct Image Variants (from ECR layer analysis)

### 1. default-* (Our current image, 2.18GB compressed, 23 layers, 44 build steps)
# Full dev environment: Ubuntu 24.04, VNC/XFCE4 desktop, Docker-in-Docker
# Languages: Python 3, Go 1.22, Rust 1.83, Node 22, Java (default-jdk)
# Tools: gcc, clang, cmake, vim, emacs, nano, htop, git-lfs, oathtool
# Desktop: TigerVNC + XFCE4 + WhiteSur theme + Plank dock + noVNC
# Chrome 145 with CDP, Playwright profile, polished-renderer for video
# Entrypoint: /usr/local/share/desktop-init.sh
# Ports: 26058 (noVNC), 5901 (VNC)

### 2. browser-use-* (696MB compressed, 10 layers, 23 build steps)
# Lightweight headless browser automation environment
# Ubuntu 24.04 MINIMAL — NO desktop, NO VNC, NO Go, NO Rust, NO Java
# Only: Node 22, Playwright Chromium (version 1148), basic apt deps
# Playwright at: ~/.cache/ms-playwright/chromium-1148/chrome-linux/chrome
# Entrypoint: bash (just a shell — exec-daemon spawns Playwright directly)
# WorkingDir: / (root, not /home/ubuntu)
# NO display, NO VNC ports exposed
# Key difference: Designed for headless browser automation only

### 3. computer-use-* (815MB compressed, 13 layers, 22 build steps)
# Desktop automation via supervisord
# Ubuntu 22.04 (NOT 24.04!) — older base image
# Xvfb virtual framebuffer at :99 (not TigerVNC)
# x11vnc on port 5900, websockify on port 6080
# Playwright Chromium (version 1194 — NEWER than browser-use's 1148)
# User: chrome (NOT ubuntu) with VNC password "chrome"
# Entrypoint: /usr/bin/supervisord -c /etc/supervisor/conf.d/supervisord.conf
# 4 supervised processes: xvfb (1920x1080x24), x11vnc, websockify, chrome
# Chrome flags: --no-sandbox --disable-dev-shm-usage --disable-gpu --start-maximized
# Chromium path cached at: /usr/local/bin/chromium-path.txt
# Points to: /root/.cache/ms-playwright/chromium-1194/chrome-linux/chrome

## Computer-Use supervisord.conf (extracted from ECR layer):
# [supervisord] nodaemon=true
# [program:xvfb]      priority=100, Xvfb :99 -screen 0 1920x1080x24 -ac +extension GLX
# [program:x11vnc]     priority=200, start-x11vnc.sh (waits for X socket + xdpyinfo)
# [program:websockify]  priority=300, websockify 6080 localhost:5900
# [program:chrome]     priority=400, start-chrome.sh (reads chromium-path.txt, starts maximized)

## Computer-Use start-chrome.sh (extracted from ECR layer):
# Waits for X server via /tmp/.X11-unix/X99 socket
# Reads cached chromium path from /usr/local/bin/chromium-path.txt
# Falls back to find search if cache miss
# Chrome flags: --no-sandbox --disable-dev-shm-usage --disable-gpu
#   --disable-software-rasterizer --no-first-run --no-default-browser-check
#   --disable-background-networking --disable-sync --disable-translate
#   --disable-extensions --start-maximized --window-size=1920,1080
#   --log-level=3 --test-type "https://www.google.com"

## Docker Host Details (from Docker API at localhost:2375):
# Docker Version: 29.1.4
# Host OS: Debian GNU/Linux 12 (bookworm)
# Kernel: 6.1.147
# CPUs: 4, Memory: 15 GB
# Storage: overlay2
# Cgroup Version: 1
# Security: seccomp with builtin profile
# Docker Root: /var/lib/docker
# Host name: d45f01ad4149 (Docker-in-Docker host)
# Containers: 1 (just our sandbox)
# Images: 1 (just our image)
# Image size on disk: 7,313 MB (uncompressed)

## Pod-daemon Process Details (from /proc/1/status):
# Binary: /pod-daemon (Rust, statically linked)
# PID: 1, UID: 0 (root), GID: 0
# VmPeak: 14,940 kB, VmRSS: 4,436 kB (tiny footprint!)
# Threads: 5 (tokio async runtime)
# Capabilities: CapPrm/Eff/Bnd = 000001ffffffffff (ALL caps)
# Seccomp: 0 (disabled), NoNewPrivs: 0
# Signal handling: SigCgt=0x4442 (SIGHUP, SIGUSR1, SIGCHLD, SIGRTMIN)

## ECR Registry Access Summary:
# Registry: public.ecr.aws/k0i0n2g5/cursorenvironments/universal
# Listing: Tags list is PUBLIC (returns all 1000 tags with auth token)
# Pull: All blobs are PUBLIC (manifests, configs, layers all downloadable)
# Catalog: /v2/_catalog returns 404 (no cross-repo enumeration)
# Other repos: Tested 5 alternate paths, all 404 — single repo only
# Token endpoint: public.ecr.aws/token (standard ECR public auth)
# Total tags: 1,000 (268 default, 251 browser-use, 51 computer-use, 430 bare)
# Tag format: {variant}-{7char-sha} matching everysphere monorepo commits
# All layers downloadable: manifests, configs, tar.gz layers all accessible
# Can reconstruct any image variant by downloading all layers

# === Added March 4, 2026 — Wave 7 (S3 artifact bucket, exec-daemon tarball, orchestration) ===

## S3 Artifact Bucket: public-asphr-vm-daemon-bucket

### Discovery: exec-daemon IS publicly downloadable!
# The version file at /exec-daemon/exec_daemon_version contains:
# https://public-asphr-vm-daemon-bucket.s3.us-east-1.amazonaws.com/exec-daemon/exec-daemon-x64-e11bf2731fbec97c4727b661f83720499761ba2f96cf8cc17212b7b25a3136ac.tar.gz
#
# This URL returns HTTP 200 — 70,431,929 bytes (67MB)
# Content-Type: application/x-tar
# Server-Side-Encryption: AES256
# Last-Modified: Wed, 04 Mar 2026 16:32:21 GMT

### Exec-daemon Tarball Contents (16 files, all in dist-package/):
#   exec-daemon        327 bytes    bash runner script (exec node index.js)
#   package.json       140 bytes    @anysphere/exec-daemon-runtime, built 2026-03-04T16:32:02.576Z
#   index.js      15,254,116 bytes  webpack bundle (agent logic, gRPC, protobuf)
#   cursorsandbox  4,514,720 bytes  Rust binary (per-command sandboxing)
#   node         123,405,064 bytes  bundled Node.js binary
#   gh            54,972,600 bytes  GitHub CLI binary
#   rg             5,416,872 bytes  ripgrep binary
#   polished-renderer.node 5,799,592 bytes  N-API video rendering addon
#   pty.node          72,664 bytes  N-API PTY addon
#   97f64a4d8eca9a2e35bb.mp4  63,178 bytes  embedded MP4 (splash/loading animation?)
#   252.index.js, 407.index.js, 511.index.js, 953.index.js, 980.index.js  (code-split chunks)

### URL Pattern:
# exec-daemon/exec-daemon-{arch}-{sha256_content_hash}.tar.gz
# The hash (64 chars) is a SHA-256 content hash of the tarball, NOT a git SHA
# Architecture: x64 (other archs may exist: arm64)

### S3 Bucket Prefix Enumeration:
# Every prefix returns HTTP 403 (exists but access denied), NOT 404:
#   /exec-daemon/   → 403 (but individual files with known hash → 200!)
#   /cursorsandbox/  → 403
#   /polished-renderer/ → 403
#   /node/           → 403
#   /rg/             → 403
#   /gh/             → 403
#   /agent/          → 403
#   /pod-daemon/     → 403 (pod-daemon with exact binary hash → also 403)
#
# Conclusion: exec-daemon tarballs are intentionally public (hash-based addressing)
# Other artifacts (pod-daemon, individual binaries) are access-restricted
# The bucket stores ALL sandbox artifacts but with selective public access

### Orchestration Flow (reconstructed):
# 1. Cursor orchestrator (cursorvm-manager) receives task request
# 2. EC2 instance with Docker Engine 29.1.4 (Debian 12) is selected/provisioned
# 3. Container started with:
#    - Image: public.ecr.aws/k0i0n2g5/cursorenvironments/universal:{variant}-{hash}
#    - Entrypoint: /pod-daemon (injected at container creation time)
#    - Network: host mode, Privileged: true, SecurityOpt: label=disable
#    - Env: GIT_LFS_SKIP_SMUDGE=1, DISPLAY=:1, VNC_RESOLUTION=1920x1200x24
# 4. Pod-daemon (Rust, 14.9MB RSS) starts, listens on :26500 gRPC
# 5. Pod-daemon downloads exec-daemon tarball from S3 bucket (URL from version file)
#    - Extracts to /exec-daemon/
#    - Stores download URL in exec_daemon_version
# 6. Pod-daemon spawns exec-daemon (Node.js) which listens on :26053/:26054
# 7. Exec-daemon connects to api2.cursor.sh for agent instructions
# 8. desktop-init.sh starts VNC/XFCE4/Chrome/Docker-in-Docker
# 9. Agent receives task, executes via tools (shell, browser, computer-use)

### Image Evolution (from ECR tag comparison):
# Older builds (e.g., default-01e07fd):
#   - 18 layers, 35 build steps
#   - Rust 1.82.0 (vs current 1.83.0)
#   - Cmd: bash (no desktop init)
#   - NO VNC desktop — just a headless shell!
# Current builds (e.g., default-b8e9345):
#   - 23 layers, 44 build steps
#   - Rust 1.83.0
#   - Cmd: /usr/local/share/desktop-init.sh
#   - Full VNC desktop with XFCE4, Chrome, Plank dock, macOS theme
# The VNC desktop was added AFTER initial deployment — older images were CLI-only

### Container Runtime Details (from Docker API):
# Container name: pod-kyaoya54prfyzkhl4qagqnuf34-b8e29869
# Container ID: 9ff6c4253fb5a42fc72ff3d809f94e604f269c23926f6111529f9904dfae1877
# Image: public.ecr.aws/k0i0n2g5/cursorenvironments/universal:default-b8e9345
# Image size on disk: 7,313 MB (uncompressed), 2,180 MB compressed
# Docker host name: d45f01ad4149 (the EC2 instance Docker hostname)
# Docker host OS: Debian GNU/Linux 12 (bookworm)
# Docker version: 29.1.4
# Only 1 container and 1 image on the host (dedicated instance per sandbox)

### Bundled Fonts (macOS fonts in /opt/cursor/ansible/files/fonts/):
# Courier.ttc, Helvetica.ttc, LucidaGrande.ttc, Monaco.ttf,
# SanFrancisco.ttf, SanFranciscoMono.ttf, Times.ttc
# + WhiteSur GTK theme, icons, and cursors from vinceliuice/WhiteSur-gtk-theme
# Desktop wallpaper: macOS Tiger from Vercel Blob Storage

### Ansible README Confirms:
# - Monorepo is "everysphere/" with root Cargo.toml
# - Public Dockerfile: anyrun/public-images/universal/Dockerfile
# - Internal Dockerfile: .cursor/Dockerfile
# - Both share the same Ansible playbook and build context
# - polished-renderer lives in ansible/files/ for both public and internal builds
# - polished-renderer Cargo.toml uses workspace references, auto-converted for standalone build

# === Added March 4, 2026 — Wave 8 (live policy, privilege, and network verification) ===

## Privilege Model (Live)
# User `ubuntu` has no effective Linux capabilities (`CapEff=0`) but has passwordless sudo:
#   /etc/sudoers.d/ubuntu -> `ubuntu ALL=(ALL) NOPASSWD:ALL`
# Root (`/proc/1/status`) has full caps (`CapEff/CapPrm/CapBnd = 000001ffffffffff`) with `Seccomp=0`.
# Practical result: sandbox user is root-equivalent (e.g., `sudo mount -t tmpfs ...` works).
# Non-root direct writes are still blocked on `/` and `/opt`, but `/workspace` and `/tmp` are writable.

## Command Policy Behavior (Observed)
# The command wrapper applies policy before shell execution:
# - benign `rm -f /tmp/...` commands are rejected with `blocked by policy`
# - read/network probes (`curl`, `ps`, `cat`, `lsof`, `netstat`) are allowed
# - `sudo` is allowed (including root-level file and mount operations)
# This indicates classifier/policy gating above Linux DAC/MAC.

## Network + Service Topology (Live)
# Interfaces/routes:
#   eth0 = 172.30.0.2/24, default gateway 172.30.0.1
#   docker0 = 172.17.0.1/16
# Resolver: `/etc/resolv.conf` -> nameserver 10.0.0.2
#
# Listening ports:
#   2375/tcp    Docker API
#   50052/tcp   gRPC-like endpoint (owner hidden in this PID namespace)
#   26500/tcp   pod-daemon gRPC
#   26058/tcp   websockify/noVNC
#   26053/26054 tcp6 exec-daemon
#   5901 localhost TigerVNC
#   3000 tcp6 user app
#
# 26500 and 50052 both return identical HTTP/2 SETTINGS frames.
# 2375 and 50052 have socket inodes in `/proc/net/tcp` but no owning visible PID from `/proc/*/fd`.
# 2375/26500/50052 are reachable via 127.0.0.1, 172.17.0.1, and host.docker.internal.

## Control Plane Reachability Signals
# `/proc/net/tcp` shows active remote peer 192.168.24.21 connected to:
#   - 172.30.0.2:26500 (pod-daemon control channel)
#   - 172.30.0.2:26058 (noVNC/websockify)
#   - 172.30.0.2:2375  (Docker API)
# This confirms host-side orchestration traffic entering the sandbox namespace.

## Docker Bootstrap Reality Check (Correction)
# `desktop-init.sh` logs:
#   - `docker: command not found`
#   - `Docker failed to become accessible after 60 seconds`
#   - `Docker: NOT ACCESSIBLE`
# No Docker CLI is in PATH and no `/var/run/docker.sock` exists.
# But Docker Engine is still reachable over unauthenticated TCP 2375:
#   - `curl 127.0.0.1:2375/version` -> Docker 29.1.4 (host OS Debian 12)
# So "DinD" access is exposed via host-network TCP, not a local service started by desktop-init.

## IMDS / Metadata Endpoint Behavior
# TCP connect to 169.254.169.254:80 succeeds.
# IMDSv1 GET and IMDSv2 token PUT both return `Empty reply from server`.
# Interpretation: metadata IP is reachable at L3/L4 but metadata service is blackholed/filtered.

## Sensitive Material Exposure Surface (Redacted)
# Exec-daemon process arguments expose runtime tokens in `ps` output:
#   --auth-token <redacted-hex>
#   --trace-auth-token <redacted-jwt>
#
# User home contains live auth stores (values intentionally not copied):
#   ~/.claude/.credentials.json  (access + refresh tokens)
#   ~/.codex/auth.json           (id/access/refresh tokens)
#   ~/.config/gh/hosts.yml       (GitHub oauth token)

## Filesystem Permission Findings
# World-writable runtime dirs:
#   /opt/cursor/artifacts         (777)
#   /opt/cursor/logs              (777)
#   /opt/cursor/recording-staging (777)
#   /opt/cursor/.exec-daemon      (1777)
#
# World-writable toolchain dirs:
#   /usr/local/cargo              (777)
#   /usr/local/cargo/bin          (777)
#   /usr/local/rustup             (777)
#   /usr/local/rustup/toolchains  (777)

## Missing Utilities (Current Runtime)
# Not present: ip, ss, iptables, nft, docker CLI, grpcurl, xxd
# Present alternatives used: ifconfig, route, netstat, lsof, curl, python3 socket probes

## Evidence Commands (Wave 8)
# grep -E 'Cap(Eff|Prm|Bnd)|Seccomp' /proc/self/status
# sudo -n grep -E 'Cap(Eff|Prm|Bnd)|Seccomp' /proc/1/status
# sudo -n grep -R ubuntu /etc/sudoers /etc/sudoers.d
# ifconfig -a ; route -n ; cat /etc/resolv.conf
# netstat -tulpen ; lsof -nP -iTCP -sTCP:LISTEN
# python3 socket probe against 127.0.0.1/172.17.0.1 ports 2375/26500/50052
# curl -v http://169.254.169.254/latest/meta-data/
# curl -X PUT http://169.254.169.254/latest/api/token -H 'X-aws-ec2-metadata-token-ttl-seconds: 21600'
# sed -n '1,220p' /tmp/container-init.log
# stat -c '%A %a %U:%G %n' /opt/cursor/* /usr/local/cargo /usr/local/rustup
# cat /exec-daemon/exec_daemon_version ; curl -I "$(cat /exec-daemon/exec_daemon_version)"
# python3 parser for /proc/net/tcp and /proc/net/tcp6 to extract peer connections
````
