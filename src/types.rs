use serde::{Deserialize, Serialize};

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
}

impl StdinPayload {
    pub fn resolve_project_path(&self) -> Option<String> {
        self.workspace
            .as_ref()
            .and_then(|workspace| workspace.current_dir.clone())
            .or_else(|| self.cwd.clone())
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line2Metrics {
    pub claude_md_count: u32,
    pub rules_count: u32,
    pub hooks_count: u32,
    pub mcp_count: u32,
    pub memory_count: u32,
    pub skills_count: u32,
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingTask {
    pub tool_use_id: String,
    pub description: String,
    pub agent_type: Option<String>,
    pub model: Option<String>,
    pub event_ts: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskItem {
    pub subject: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub description: String,
    pub agent_type: Option<String>,
    pub started_at: Option<u64>,
    pub model: Option<String>,
    pub completed_at: Option<u64>,
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
    pub plan_type: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoSummary {
    pub text: String,
    pub pending: usize,
    pub completed: usize,
    pub total: usize,
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
            },
            line2: Line2Metrics {
                claude_md_count: 0,
                rules_count: 0,
                memory_count: 0,
                hooks_count: 0,
                mcp_count: 0,
                skills_count: 0,
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
            quota: QuotaMetrics::default(),
        }
    }
}
