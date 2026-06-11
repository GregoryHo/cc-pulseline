use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StdinPayload {
    pub session_id: Option<String>,
    pub model: Option<ModelInfo>,
    pub output_style: Option<OutputStyleInfo>,
    pub version: Option<String>,
    pub cwd: Option<String>,
    pub workspace: Option<WorkspaceInfo>,
    pub context_window: Option<ContextWindow>,
    pub cost: Option<CostInfo>,
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub rate_limits: Option<RateLimits>,
    #[serde(default)]
    pub agent: Option<AgentInfo>,
    #[serde(default)]
    pub worktree: Option<WorktreeInfo>,
    /// Active effort level (CC 2.1.119+): "low" | "medium" | "high" | "xhigh" | "max" | "auto".
    #[serde(default)]
    pub effort: Option<EffortInfo>,
    /// Thinking mode toggle (CC 2.1.119+).
    #[serde(default)]
    pub thinking: Option<ThinkingInfo>,
}

impl StdinPayload {
    pub fn resolve_project_path(&self) -> Option<String> {
        // In worktree sessions, prefer the original project directory so that
        // env/memory lookups and session keying use the real repo, not the temp worktree.
        self.worktree
            .as_ref()
            .and_then(|wt| wt.original_cwd.clone())
            .or_else(|| {
                self.workspace
                    .as_ref()
                    .and_then(|workspace| workspace.current_dir.clone())
            })
            .or_else(|| self.cwd.clone())
    }

    /// True when this session is inside a git worktree — via either the explicit
    /// `--worktree` flag (CC 2.1.69) or the passive `workspace.git_worktree` field
    /// (CC 2.1.97+). Accepts any non-null `git_worktree` value defensively.
    pub fn is_in_worktree(&self) -> bool {
        self.worktree.is_some()
            || self
                .workspace
                .as_ref()
                .is_some_and(|w| w.git_worktree.as_ref().is_some_and(|v| !v.is_null()))
    }

    pub fn resolve_project_path_display(&self) -> String {
        let raw_path = self
            .resolve_project_path()
            .unwrap_or_else(|| "unknown".to_string());

        // Cross-platform home directory detection
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok();

        if let Some(home_dir) = home {
            if raw_path.starts_with(&home_dir) {
                return raw_path.replacen(&home_dir, "~", 1);
            }
        }

        raw_path
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelInfo {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OutputStyleInfo {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WorkspaceInfo {
    pub current_dir: Option<String>,
    /// CC 2.1.97+: present whenever the current directory sits in a linked git
    /// worktree (independent of the `--worktree` CLI flag, which is `payload.worktree`).
    ///
    /// Type kept as `serde_json::Value` so a future CC schema change (bool → object)
    /// does not break parsing — the renderer only checks for non-null presence.
    #[serde(default)]
    pub git_worktree: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ContextWindow {
    pub context_window_size: Option<u64>,
    pub used_percentage: Option<u64>,
    pub current_usage: Option<CurrentUsage>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CurrentUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CostInfo {
    pub total_cost_usd: Option<f64>,
    pub total_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RateLimits {
    pub five_hour: Option<RateLimitWindow>,
    pub seven_day: Option<RateLimitWindow>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RateLimitWindow {
    pub used_percentage: Option<f64>,
    /// Unix epoch seconds when this window resets.
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentInfo {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub agent_type: Option<String>,
}

/// Active effort level for the model (Claude Code 2.1.119+).
///
/// Known values: `"low"`, `"medium"`, `"high"`, `"xhigh"`, `"max"`, `"auto"`.
/// Unknown values are preserved as-is — Claude Code adds levels frequently
/// (e.g. `xhigh` was added in 2.1.111), and the renderer falls back gracefully.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EffortInfo {
    pub level: Option<String>,
}

/// Thinking-mode toggle (Claude Code 2.1.119+).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ThinkingInfo {
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WorktreeInfo {
    pub name: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
    pub original_cwd: Option<String>,
    pub original_branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line1Metrics {
    pub model: String,
    pub output_style: String,
    pub claude_code_version: String,
    pub project_path: String,
    pub git_branch: String,
    pub git_dirty: bool,
    pub git_ahead: u32,
    pub git_behind: u32,
    pub git_modified: u32,
    pub git_added: u32,
    pub git_deleted: u32,
    pub git_untracked: u32,
    pub agent_name: Option<String>,
    pub in_worktree: bool,
    /// Effort level from stdin `effort.level` (CC 2.1.119+).
    pub effort_level: Option<String>,
    /// Thinking mode flag from stdin `thinking.enabled` (CC 2.1.119+).
    pub thinking_enabled: Option<bool>,
}

impl Line1Metrics {
    /// True when a real branch was resolved. `"unknown"` is the
    /// `GitSnapshot::default()` sentinel produced outside a repository —
    /// the git segment is hidden rather than rendering a literal "unknown".
    pub fn has_git_branch(&self) -> bool {
        !self.git_branch.is_empty() && self.git_branch != "unknown"
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line2Metrics {
    pub claude_md_count: u32,
    /// Root-level AGENTS.md presence (repo-root only, see `count_agents_md`).
    pub agents_md_count: u32,
    pub rules_count: u32,
    pub hooks_count: u32,
    pub mcp_count: u32,
    pub memory_count: u32,
    pub skills_count: u32,
    /// Count of enabled Claude Code plugins (CC 2.0.12+).
    /// Sourced from `~/.claude/plugins/installed_plugins.json` cross-referenced
    /// with `enabledPlugins` in `~/.claude/settings.json`.
    pub plugins_count: u32,
    pub elapsed_minutes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Line3Metrics {
    pub context_window_size: Option<u64>,
    pub context_used_percentage: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_cost_usd: Option<f64>,
    pub total_duration_ms: Option<u64>,
    /// Output speed in tokens/second (independently computed, NOT from payload).
    #[serde(default)]
    pub output_speed_toks_per_sec: Option<f64>,
}

impl Line3Metrics {
    /// Cache hit rate as a percentage: `read / (input + creation + read)`.
    ///
    /// `None` means no cache signal at all (including the first-tick
    /// `current_usage: null` case) — callers render dashes rather than a
    /// misleading 0%.
    pub fn cache_hit_pct(&self) -> Option<f64> {
        let read = self.cache_read_tokens.unwrap_or(0);
        let creation = self.cache_creation_tokens.unwrap_or(0);
        if read + creation == 0 {
            return None;
        }
        let denom = self.input_tokens.unwrap_or(0) + creation + read;
        Some(read as f64 * 100.0 / denom as f64)
    }

    /// Returns true if any field has a value (not all None).
    pub fn has_data(&self) -> bool {
        self.context_window_size.is_some()
            || self.context_used_percentage.is_some()
            || self.input_tokens.is_some()
            || self.output_tokens.is_some()
            || self.cache_creation_tokens.is_some()
            || self.cache_read_tokens.is_some()
            || self.total_cost_usd.is_some()
            || self.total_duration_ms.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSummary {
    pub id: String,
    pub name: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletedToolCount {
    pub name: String,
    pub count: u32,
    #[serde(default)]
    pub last_completed_at: Option<u64>,
    /// Number of times this tool returned an error result (`is_error: true`).
    #[serde(default)]
    pub failed: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingTask {
    pub tool_use_id: String,
    pub description: String,
    pub agent_type: Option<String>,
    pub model: Option<String>,
    pub event_ts: Option<u64>,
    /// Anthropic API `message.id` of the assistant turn that emitted the
    /// Agent tool_use. Propagated to `AgentSummary` on link so batch
    /// detection can group agents from the same turn. `None` when the
    /// transcript event lacks message envelope (Path 2/3 fallbacks).
    #[serde(default)]
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskItem {
    pub subject: String,
    pub status: String,
    #[serde(default)]
    pub active_form: Option<String>,
    #[serde(default)]
    pub started_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub description: String,
    pub agent_type: Option<String>,
    pub started_at: Option<u64>,
    pub model: Option<String>,
    pub completed_at: Option<u64>,
    /// Anthropic API message ID of the assistant turn that spawned this
    /// agent. Agents sharing this ID belong to one parallel batch (filled
    /// by `providers/transcript.rs` Path-1 dispatcher). `None` for legacy
    /// cache files or when CC's JSONL schema drifts.
    #[serde(default)]
    pub message_id: Option<String>,
    /// Runtime agentId from CC's async-agent `toolUseResult.agentId`.
    /// `Some` once the parent transcript has emitted the async_launched
    /// tool_result for this agent; keyed against
    /// `~/.claude/projects/<encoded-cwd>/<parent>/subagents/agent-<id>.jsonl`
    /// for sub-agent transcript tailing.
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Total session duration reported in the `toolUseResult` completion envelope (ms).
    #[serde(default)]
    pub total_duration_ms: Option<u64>,
    /// Total token count from the `toolUseResult` completion envelope.
    #[serde(default)]
    pub total_tokens: Option<u64>,
    /// Total tool-use count from the `toolUseResult` completion envelope.
    #[serde(default)]
    pub total_tool_use_count: Option<u64>,
}

impl AgentSummary {
    pub fn is_completed(&self) -> bool {
        self.completed_at.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuotaMetrics {
    pub five_hour_pct: Option<f64>,
    pub five_hour_reset_minutes: Option<u64>,
    pub seven_day_pct: Option<f64>,
    pub seven_day_reset_minutes: Option<u64>,
}

impl QuotaMetrics {
    /// Convert stdin `rate_limits` into render-ready metrics.
    /// Converts absolute `resets_at` (epoch seconds) to relative minutes-from-now.
    /// Accepts `now_secs` for testability (pure function).
    pub fn from_rate_limits(rate_limits: &RateLimits, now_secs: u64) -> Self {
        let reset_to_minutes =
            |reset_secs: u64| -> u64 { reset_secs.saturating_sub(now_secs) / 60 };

        Self {
            five_hour_pct: rate_limits
                .five_hour
                .as_ref()
                .and_then(|w| w.used_percentage),
            five_hour_reset_minutes: rate_limits
                .five_hour
                .as_ref()
                .and_then(|w| w.resets_at)
                .map(reset_to_minutes),
            seven_day_pct: rate_limits
                .seven_day
                .as_ref()
                .and_then(|w| w.used_percentage),
            seven_day_reset_minutes: rate_limits
                .seven_day
                .as_ref()
                .and_then(|w| w.resets_at)
                .map(reset_to_minutes),
        }
    }

    /// Returns true if any quota percentage data is present.
    pub fn has_data(&self) -> bool {
        self.five_hour_pct.is_some() || self.seven_day_pct.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoInProgressItem {
    pub text: String,
    #[serde(default)]
    pub started_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoSummary {
    pub text: String,
    pub pending: usize,
    pub completed: usize,
    pub total: usize,
    #[serde(default)]
    pub in_progress_items: Vec<TodoInProgressItem>,
    #[serde(default)]
    pub all_done: bool,
    #[serde(default)]
    pub is_task_api: bool,
    /// When the TODO line is aggregated from background sub-agents
    /// (Task-tool dispatch), this is the number of contributing agents.
    /// `None` for the user's own TODO. Renderer surfaces it as
    /// "(N agents)" so the user knows the source.
    #[serde(default)]
    pub sub_agent_count: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderFrame {
    pub line1: Line1Metrics,
    pub line2: Line2Metrics,
    pub line3: Line3Metrics,
    pub tools: Vec<ToolSummary>,
    pub completed_tools: Vec<CompletedToolCount>,
    pub agents: Vec<AgentSummary>,
    pub todo: Option<TodoSummary>,
    pub quota: QuotaMetrics,
    /// Rolling CTX% history (oldest → newest), each entry `(pct, epoch_ms)`.
    /// Read by the ledger sparkline (and by `render_context_visual` when a
    /// flat layout opts in to `+sparkline`). Populated by `PulseLineRunner`
    /// from `SessionState.ctx_history`.
    pub ctx_history: Vec<(u8, u64)>,
    /// Number of `compact_boundary` events seen this session.
    pub compact_count: u32,
    /// Epoch-ms of the most recent `api_error` event (if within 30s TTL).
    pub last_api_error_ms: Option<u64>,
}

impl RenderFrame {
    pub fn from_payload(payload: &StdinPayload) -> Self {
        let model = payload
            .model
            .as_ref()
            .and_then(|model| model.display_name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let output_style = payload
            .output_style
            .as_ref()
            .and_then(|style| style.name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let claude_code_version = payload
            .version
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let project_path = payload.resolve_project_path_display();

        let ctx = payload.context_window.as_ref();
        let usage = ctx.and_then(|c| c.current_usage.as_ref());

        let elapsed_minutes = payload
            .cost
            .as_ref()
            .and_then(|cost| cost.total_duration_ms)
            .unwrap_or(0)
            / 60_000;

        Self {
            line1: Line1Metrics {
                model,
                output_style,
                claude_code_version,
                project_path,
                git_branch: "unknown".to_string(),
                git_dirty: false,
                git_ahead: 0,
                git_behind: 0,
                git_modified: 0,
                git_added: 0,
                git_deleted: 0,
                git_untracked: 0,
                agent_name: payload.agent.as_ref().and_then(|a| a.name.clone()),
                in_worktree: payload.is_in_worktree(),
                effort_level: payload.effort.as_ref().and_then(|e| e.level.clone()),
                thinking_enabled: payload.thinking.as_ref().and_then(|t| t.enabled),
            },
            line2: Line2Metrics {
                claude_md_count: 0,
                agents_md_count: 0,
                rules_count: 0,
                memory_count: 0,
                hooks_count: 0,
                mcp_count: 0,
                skills_count: 0,
                plugins_count: 0,
                elapsed_minutes,
            },
            line3: Line3Metrics {
                context_window_size: ctx.and_then(|c| c.context_window_size),
                context_used_percentage: ctx.and_then(|c| c.used_percentage),
                input_tokens: usage.and_then(|usage| usage.input_tokens),
                output_tokens: usage.and_then(|usage| usage.output_tokens),
                cache_creation_tokens: usage.and_then(|usage| usage.cache_creation_input_tokens),
                cache_read_tokens: usage.and_then(|usage| usage.cache_read_input_tokens),
                total_cost_usd: payload.cost.as_ref().and_then(|cost| cost.total_cost_usd),
                total_duration_ms: payload
                    .cost
                    .as_ref()
                    .and_then(|cost| cost.total_duration_ms),
                output_speed_toks_per_sec: None,
            },
            tools: Vec::new(),
            completed_tools: Vec::new(),
            agents: Vec::new(),
            todo: None,
            quota: payload
                .rate_limits
                .as_ref()
                .map(|rl| {
                    let now_secs = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    QuotaMetrics::from_rate_limits(rl, now_secs)
                })
                .unwrap_or_default(),
            ctx_history: Vec::new(),
            compact_count: 0,
            last_api_error_ms: None,
        }
    }
}
