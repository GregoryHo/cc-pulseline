# Claude Code Integration

## Hook Lifecycle

- Claude Code pipes JSON to cc-pulseline's stdin on each UI state change
- ~300ms debounce; in-flight executions cancelled on rapid updates
- Binary must complete fast (p95 < 50ms) or output goes blank
- Errors in the binary result in a blank statusline (no crash propagation)
- The binary is stateless per invocation — session continuity via disk cache

## Stdin Payload Schema

All fields are `Option<T>` — missing fields fall back to defaults, never panic.

```rust
StdinPayload {
    session_id: Option<String>,         // Unique session identifier
    model: Option<ModelInfo>,           // Active model (id + display_name)
    output_style: Option<OutputStyleInfo>, // Style name (e.g., "explanatory")
    version: Option<String>,            // Claude Code version (e.g., "2.1.37")
    cwd: Option<String>,                // Current working directory (fallback)
    workspace: Option<WorkspaceInfo>,   // workspace.current_dir (preferred project path)
    context_window: Option<ContextWindow>, // Context budget + token usage
    cost: Option<CostInfo>,             // Session cost + duration
    transcript_path: Option<String>,    // Path to JSONL transcript file
}
```

### Nested Structs

```
ModelInfo          { id, display_name }
OutputStyleInfo    { name }
WorkspaceInfo      { current_dir }
ContextWindow      { context_window_size, used_percentage, current_usage }
CurrentUsage       { input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens }
CostInfo           { total_cost_usd, total_duration_ms }
```

### Important Behaviors

- `current_usage` is `null` before the first API call in a session
- `used_percentage` is input-only (excludes output tokens from the percentage)
- Project path: `workspace.current_dir` is preferred; `cwd` is the fallback
- Empty stdin (`""`) is treated as `{}` — all fields default gracefully

## Transcript File

- Path provided via `transcript_path` in the stdin payload
- Format: JSONL (one JSON object per line), appended incrementally by Claude Code
- cc-pulseline reads incrementally via seek-based offsets (never re-reads entire file)
- Contains tool use/result events, agent progress events, and todo state
- See `providers/transcript.rs` for the three-path event dispatcher

## Output Contract

- Each line to stdout = one statusline row
- ANSI 256-color codes supported (disabled via `NO_COLOR` env or config)
- Multiple lines output (L1 identity, L2 config, L3 budget, L4+ activity)
- Empty output on error — Claude Code displays nothing rather than stale data

## Settings Configuration

### Statusline Registration

User scope (`~/.claude/settings.json`):
```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/pulseline/cc-pulseline"
  }
}
```

Project scope (`{root}/.claude/settings.json`) uses the same structure.

### Install Path Convention

- Binary: `~/.claude/pulseline/cc-pulseline`
- User config: `~/.claude/pulseline/config.toml`
- Project config: `{root}/.claude/pulseline.toml`
- Cache files: `{temp_dir}/cc-pulseline-{hash}.json`

## Debugging

- **Standalone test**: `echo '{"session_id":"test","version":"1.0"}' | cc-pulseline`
- **Validate config**: `cc-pulseline --check`
- **Show merged config**: `cc-pulseline --print`
- **Cache inspection**: look in `$TMPDIR/cc-pulseline-*.json`
- **Transcript replay**: pipe a fixture payload with `transcript_path` pointing to a `.jsonl` file

## Environment Variables

- `NO_COLOR` — disables all ANSI color output (standard convention)
- `COLUMNS` — terminal width for layout degradation
- `PULSELINE_THEME` — `dark` (default) or `light` theme selection
- `HOME` / `USERPROFILE` — used for `~` path abbreviation and user config location

## MCP Relationship

- MCP servers are not directly related to the statusline hook mechanism
- cc-pulseline discovers MCPs by scanning Claude Code settings files (env provider)
- The MCP count displayed on L2 reflects configured servers, not active connections
- Active MCP tool invocations are tracked indirectly via transcript parsing
