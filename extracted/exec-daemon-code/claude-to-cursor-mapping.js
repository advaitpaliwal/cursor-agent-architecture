// Extracted from exec-daemon webpack bundle (index.js)
// @anysphere/exec-daemon-runtime - Built 2026-03-04
// Source: ../hooks/dist/claude-code-types.js

/**
 * Types for Claude Code hooks configuration.
 * These types represent the structure of settings.json files used by Claude Code.
 *
 * @see https://docs.anthropic.com/en/docs/claude-code/hooks
 */

/**
 * Mapping from Claude Code event names to Cursor hook steps.
 */
const CLAUDE_EVENT_TO_CURSOR_STEP = {
    PreToolUse: "preToolUse",
    PermissionRequest: null, // Not supported in Cursor
    PostToolUse: "postToolUse",
    UserPromptSubmit: "beforeSubmitPrompt",
    Stop: "stop",
    SubagentStop: "subagentStop",
    SessionStart: "sessionStart",
    SessionEnd: "sessionEnd",
    PreCompact: "preCompact",
    Notification: null, // Not supported in Cursor
};

/**
 * Mapping from Claude Code tool names to Cursor NAL tool names.
 * null means the tool is not supported.
 */
const CLAUDE_TOOL_TO_CURSOR_TOOL = {
    Bash: "Shell",
    Read: "Read",
    Write: "Write",
    Edit: "Write", // Edit maps to Write
    Glob: null, // Not exposed in CLI
    Grep: "Grep",
    WebFetch: "WebFetch",
    WebSearch: "WebSearch",
    Task: "Task", // preToolUse/postToolUse supported
};

/**
 * Claude Code tools that are not supported in Cursor.
 * Used for logging warnings.
 */
const UNSUPPORTED_CLAUDE_TOOLS = ["Glob"];

/**
 * Claude Code events that are not supported in Cursor.
 */
const UNSUPPORTED_CLAUDE_EVENTS = [
    "Notification",
    "PermissionRequest",
];

// MCP tool transformation: mcp__<server>__<tool> -> MCP:<tool>
// Wildcard matching: "" or "*" -> "*"
// Combined patterns: "Bash|Write" -> "Shell|Write"
