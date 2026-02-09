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
    pub fn project_path(&self) -> Option<String> {
        self.resolve_project_path()
    }

    pub fn model_display(&self) -> String {
        self.model
            .as_ref()
            .and_then(|model| model.display_name.clone().or_else(|| model.id.clone()))
            .unwrap_or_else(|| "unknown".to_string())
    }

    pub fn output_style_name(&self) -> String {
        self.output_style
            .as_ref()
            .and_then(|style| style.name.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line2Metrics {
    pub claude_md_count: u32,
    pub rules_count: u32,
    pub hooks_count: u32,
    pub mcp_count: u32,
    pub skills_count: u32,
    pub elapsed_minutes: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line3Metrics {
    pub context_window_size: Option<u64>,
    pub context_used_percentage: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub total_cost_usd: Option<f64>,
    pub total_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolSummary {
    pub id: String,
    pub name: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletedToolCount {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentSummary {
    pub id: String,
    pub description: String,
    pub agent_type: Option<String>,
    pub started_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

        let usage = payload
            .context_window
            .as_ref()
            .and_then(|context| context.current_usage.as_ref());

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
            },
            line2: Line2Metrics {
                claude_md_count: 0,
                rules_count: 0,
                hooks_count: 0,
                mcp_count: 0,
                skills_count: 0,
                elapsed_minutes,
            },
            line3: Line3Metrics {
                context_window_size: payload
                    .context_window
                    .as_ref()
                    .and_then(|context| context.context_window_size),
                context_used_percentage: payload
                    .context_window
                    .as_ref()
                    .and_then(|context| context.used_percentage),
                input_tokens: usage.and_then(|usage| usage.input_tokens),
                output_tokens: usage.and_then(|usage| usage.output_tokens),
                cache_creation_tokens: usage.and_then(|usage| usage.cache_creation_input_tokens),
                cache_read_tokens: usage.and_then(|usage| usage.cache_read_input_tokens),
                total_cost_usd: payload.cost.as_ref().and_then(|cost| cost.total_cost_usd),
                total_duration_ms: payload
                    .cost
                    .as_ref()
                    .and_then(|cost| cost.total_duration_ms),
            },
            tools: Vec::new(),
            completed_tools: Vec::new(),
            agents: Vec::new(),
            todo: None,
        }
    }
}
