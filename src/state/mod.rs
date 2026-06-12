pub mod cache;

use std::{
    collections::{HashMap, VecDeque},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    providers::{EnvSnapshot, GitSnapshot},
    types::{
        AgentSummary, CompletedToolCount, Line3Metrics, PendingTask, TaskItem, TodoInProgressItem,
        TodoSummary, ToolSummary,
    },
};
use cache::{CacheEntry, SessionCache, CACHE_TTL_MS};
use serde::{Deserialize, Serialize};

const MAX_RECENT_TOOLS_CAP: usize = 10;

/// Maximum number of sub-agent transcripts tailed concurrently. Each adds
/// one open+seek+read per statusline tick; the cap keeps the worst-case
/// file-IO bounded under the p95 < 50ms render budget. Drop oldest by
/// `last_event_ts` on overflow.
pub const MAX_SUBAGENT_TAILS: usize = 6;

/// Upper bound on `task_items` retained per sub-agent. Pathological agents
/// doing thousands of TaskCreate calls would otherwise bloat the session
/// cache file (every entry serializes to disk each tick). On overflow,
/// drop the lowest-numbered task ids (FIFO oldest-first).
pub const MAX_SUBAGENT_TASKS: usize = 200;

/// Per-sub-agent transcript state. Each Claude Code agent dispatched via
/// the Task tool gets its own JSONL transcript at
/// `<parent_dir>/subagents/agent-<agentId>.jsonl`; this struct holds the
/// incremental tail state so cc-pulseline can surface TODO progress from
/// inside the sub-agent. Aggregation happens at snapshot time
/// (`snapshot_from_state`), not during ingestion — each sub-agent's task
/// universe stays isolated.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubAgentTranscriptState {
    pub offset: u64,
    #[serde(default)]
    pub task_items: HashMap<String, TaskItem>,
    #[serde(default)]
    pub task_counter: u32,
    /// Fallback for sub-agents that use the legacy TodoWrite path (not
    /// TaskCreate/TaskUpdate). Aggregator prefers this when present.
    #[serde(default)]
    pub legacy_todo: Option<TodoSummary>,
    #[serde(default)]
    pub last_event_ts: Option<u64>,
}

/// Canonical builder for the Task-API `TodoSummary`. Used by both the
/// parent session's `rebuild_todo_from_tasks` and each sub-agent's
/// `derived_todo`; aggregator paths reuse the same fields via direct
/// construction. Returns `None` when `task_items` is empty so callers
/// can fall through to other sources (e.g. legacy TodoWrite).
pub fn build_todo_from_tasks(task_items: &HashMap<String, TaskItem>) -> Option<TodoSummary> {
    if task_items.is_empty() {
        return None;
    }
    let total = task_items.len();
    let completed = task_items
        .values()
        .filter(|item| item.status == "completed")
        .count();
    let pending = total.saturating_sub(completed);

    let mut in_progress_items: Vec<TodoInProgressItem> = task_items
        .values()
        .filter(|item| item.status == "in_progress")
        .map(|item| TodoInProgressItem {
            text: item
                .active_form
                .clone()
                .unwrap_or_else(|| item.subject.clone()),
            started_at: item.started_at,
        })
        .collect();
    in_progress_items.sort_by_key(|item| item.started_at.unwrap_or(u64::MAX));

    let all_done = pending == 0 && completed > 0;
    Some(TodoSummary {
        text: format!("{completed}/{total} done, {pending} pending"),
        pending,
        completed,
        total,
        in_progress_items,
        all_done,
        is_task_api: true,
        sub_agent_count: None,
    })
}

impl SubAgentTranscriptState {
    pub fn create_task_item(&mut self, subject: String, active_form: Option<String>) {
        self.task_counter += 1;
        let id = self.task_counter.to_string();
        self.task_items.insert(
            id,
            TaskItem {
                subject,
                status: "pending".to_string(),
                active_form,
                started_at: None,
            },
        );
        // `task_counter` only grows; the oldest still-resident ids are
        // therefore the smallest numerically. Evict in that order until
        // the cap is satisfied.
        while self.task_items.len() > MAX_SUBAGENT_TASKS {
            let oldest = self
                .task_items
                .keys()
                .filter_map(|k| k.parse::<u32>().ok().map(|n| (n, k.clone())))
                .min_by_key(|(n, _)| *n)
                .map(|(_, k)| k);
            match oldest {
                Some(k) => {
                    self.task_items.remove(&k);
                }
                None => break,
            }
        }
    }

    /// Mirror of `SessionState::update_task_item` scoped to one sub-agent.
    pub fn update_task_item(&mut self, task_id: &str, status: &str) {
        if status == "deleted" {
            self.task_items.remove(task_id);
        } else if let Some(item) = self.task_items.get_mut(task_id) {
            if status == "in_progress" && item.status != "in_progress" {
                item.started_at = Some(cache::now_epoch_ms());
            }
            item.status = status.to_string();
        }
    }

    pub fn set_legacy_todo(&mut self, todo: Option<TodoSummary>) {
        self.legacy_todo = todo;
    }

    /// Sub-agent TODO from Task API items, falling back to the legacy
    /// TodoWrite summary when there are no Task items.
    pub fn derived_todo(&self) -> Option<TodoSummary> {
        build_todo_from_tasks(&self.task_items).or_else(|| self.legacy_todo.clone())
    }
}

/// Maximum number of CTX% samples retained for the ledger sparkline. The
/// widget draws 12 (6 braille cells × 2 samples each); the surplus lets
/// the adaptive 1-minute window expand when samples arrive in bursts.
pub const MAX_CTX_HISTORY: usize = 30;

/// Maximum number of cache hit-rate samples retained for the cache-trend
/// sparkline. Same rationale as `MAX_CTX_HISTORY`: the sparkline widget
/// draws 12 samples (6 braille cells × 2); the surplus absorbs bursts.
pub const MAX_CACHE_HISTORY: usize = 30;

#[derive(Debug, Clone, Default)]
pub struct SessionState {
    pub last_transcript_offset: u64,
    pub last_transcript_path: Option<String>,
    pub last_transcript_poll: Option<Instant>,
    pub active_tools: Vec<ToolSummary>,
    pub recent_tools: Vec<ToolSummary>,
    pub active_agents: Vec<AgentSummary>,
    pub completed_agents: Vec<AgentSummary>,
    pub completed_tool_counts: HashMap<String, (u32, u64)>,
    /// Error count per tool name (subset of `completed_tool_counts`).
    pub completed_tool_failures: HashMap<String, u32>,
    pub todo: Option<TodoSummary>,
    // Agent linking: Agent tool_use → agent_progress ID linking
    pub pending_tasks: Vec<PendingTask>,
    pub task_agent_links: HashMap<String, String>, // tool_use_id → agentId
    // Todo tracking: TaskCreate/TaskUpdate stateful accumulation
    pub task_items: HashMap<String, TaskItem>,
    pub task_counter: u32,
    pub cached_env: Option<(String, EnvSnapshot)>,
    pub cached_git: Option<(String, GitSnapshot)>,
    pub cached_line3: Option<Line3Metrics>,
    // Token speed tracking (output only)
    pub last_output_tokens: Option<u64>,
    pub last_output_token_time_ms: Option<u64>,
    pub output_speed_toks_per_sec: Option<f64>,
    /// Sparkline source: rolling window of `(pct, epoch_ms)` samples.
    /// Timestamps drive the ledger's adaptive 1-minute window + velocity-
    /// based aurora coloring.
    pub ctx_history: VecDeque<(u8, u64)>,
    /// Cache-trend source: rolling window of `(cache_hit_pct, epoch_ms)`
    /// samples, oldest→newest.
    pub cache_history: VecDeque<(u8, u64)>,
    /// Cumulative `cache_read_input_tokens` accumulated from transcript
    /// usage events. Session-cumulative: NOT cleared by `compact_boundary`.
    pub cache_read_total: u64,
    /// Dedupe key: the Anthropic `message.id` of the last usage event
    /// accumulated. Streaming chunks of one API call repeat the same id
    /// with identical usage; chunks are contiguous, so one remembered id
    /// suffices.
    pub last_usage_message_id: Option<String>,
    /// Per-sub-agent transcript tail state, keyed by runtime `agentId`.
    /// Populated when the parent transcript surfaces a `toolUseResult` with
    /// `status == "async_launched"`; pruned when the corresponding agent
    /// reaches a terminal status. See `MAX_SUBAGENT_TAILS` for the cap.
    pub sub_agents: HashMap<String, SubAgentTranscriptState>,
    /// Number of context compaction events (`system/compact_boundary`) seen
    /// since the transcript started. Displayed as `⟳N` on L3.
    pub compact_count: u32,
    /// Timestamp (epoch ms) of the last `system/api_error` event seen.
    /// Cleared when the transcript changes. Used for the 30s TTL badge.
    pub last_api_error_ms: Option<u64>,
}

impl SessionState {
    pub fn reset_transcript_if_path_changed(&mut self, transcript_path: &str) {
        if self.last_transcript_path.as_deref() != Some(transcript_path) {
            self.last_transcript_path = Some(transcript_path.to_string());
            self.last_transcript_offset = 0;
            self.last_transcript_poll = None;
            self.active_tools.clear();
            self.recent_tools.clear();
            self.active_agents.clear();
            self.completed_agents.clear();
            self.completed_tool_counts.clear();
            self.completed_tool_failures.clear();
            self.todo = None;
            self.pending_tasks.clear();
            self.task_agent_links.clear();
            self.task_items.clear();
            self.task_counter = 0;
            self.cached_line3 = None;
            self.last_output_tokens = None;
            self.last_output_token_time_ms = None;
            self.output_speed_toks_per_sec = None;
            self.ctx_history.clear();
            self.cache_history.clear();
            self.cache_read_total = 0;
            self.last_usage_message_id = None;
            self.sub_agents.clear();
            self.compact_count = 0;
            self.last_api_error_ms = None;
        }
    }

    /// Push a fresh CTX% sample stamped with `ts_ms`, discarding the oldest
    /// when at capacity.
    pub fn push_ctx_sample(&mut self, pct: u8, ts_ms: u64) {
        if self.ctx_history.len() == MAX_CTX_HISTORY {
            self.ctx_history.pop_front();
        }
        self.ctx_history.push_back((pct.min(100), ts_ms));
    }

    /// Push a fresh cache hit-rate sample stamped with `ts_ms`, discarding
    /// the oldest when at capacity.
    pub fn push_cache_sample(&mut self, pct: u8, ts_ms: u64) {
        if self.cache_history.len() == MAX_CACHE_HISTORY {
            self.cache_history.pop_front();
        }
        self.cache_history.push_back((pct.min(100), ts_ms));
    }

    /// Compute output token speed from successive payload snapshots.
    /// Returns the current speed estimate, or keeps the last known value if the
    /// delta window exceeds 2 seconds (likely idle/paused).
    /// When `current_tokens` is `None`, returns the last known speed without
    /// corrupting the time anchor (bug #2 fix).
    pub fn update_output_speed(&mut self, current_tokens: Option<u64>) -> Option<f64> {
        let curr = match current_tokens {
            Some(c) => c,
            None => return self.output_speed_toks_per_sec, // early-return: don't touch state
        };
        let now_ms = cache::now_epoch_ms();
        let result = match (self.last_output_tokens, self.last_output_token_time_ms) {
            (Some(prev), Some(prev_time)) => {
                let delta_ms = now_ms.saturating_sub(prev_time);
                let delta_toks = curr.saturating_sub(prev);
                if delta_ms > 0 && delta_ms <= 2000 && delta_toks > 0 {
                    Some(delta_toks as f64 / delta_ms as f64 * 1000.0)
                } else {
                    self.output_speed_toks_per_sec // keep last known
                }
            }
            _ => None,
        };
        self.last_output_tokens = Some(curr);
        self.last_output_token_time_ms = Some(now_ms);
        self.output_speed_toks_per_sec = result;
        result
    }

    pub fn cached_env_for(&self, cwd: &str) -> Option<EnvSnapshot> {
        self.cached_env.as_ref().and_then(|(path, snapshot)| {
            if path == cwd {
                Some(snapshot.clone())
            } else {
                None
            }
        })
    }

    pub fn set_cached_env(&mut self, cwd: String, snapshot: EnvSnapshot) {
        self.cached_env = Some((cwd, snapshot));
    }

    pub fn cached_git_for(&self, cwd: &str) -> Option<GitSnapshot> {
        self.cached_git.as_ref().and_then(|(path, snapshot)| {
            if path == cwd {
                Some(snapshot.clone())
            } else {
                None
            }
        })
    }

    pub fn set_cached_git(&mut self, cwd: String, snapshot: GitSnapshot) {
        self.cached_git = Some((cwd, snapshot));
    }

    pub fn upsert_tool(&mut self, id: String, name: String, target: Option<String>) {
        if let Some(position) = self.active_tools.iter().position(|tool| tool.id == id) {
            self.active_tools.remove(position);
        }
        let tool = ToolSummary { id, name, target };
        self.active_tools.push(tool.clone());

        // recent_tools: display list (dedup, push, cap)
        self.recent_tools.retain(|t| t.id != tool.id);
        self.recent_tools.push(tool);
        if self.recent_tools.len() > MAX_RECENT_TOOLS_CAP {
            self.recent_tools
                .drain(..self.recent_tools.len() - MAX_RECENT_TOOLS_CAP);
        }
    }

    pub fn remove_tool_with_error(&mut self, id: &str, event_ts: Option<u64>, is_error: bool) {
        if let Some(tool) = self.active_tools.iter().find(|t| t.id == id) {
            self.record_tool_completion(&tool.name.clone(), event_ts, is_error);
        }
        self.active_tools.retain(|tool| tool.id != id);
        // recent_tools: intentionally NOT touched — tool stays visible until displaced
    }

    pub fn record_tool_completion(&mut self, name: &str, event_ts: Option<u64>, is_error: bool) {
        let ts = event_ts.unwrap_or_else(cache::now_epoch_ms);
        let entry = self
            .completed_tool_counts
            .entry(name.to_string())
            .or_insert((0, 0));
        entry.0 += 1;
        entry.1 = ts;
        if is_error {
            *self
                .completed_tool_failures
                .entry(name.to_string())
                .or_insert(0) += 1;
        }
    }

    /// Hybrid scoring: count + recency bonus (decays over 2 minutes).
    /// Recently used tools float up even with low counts.
    pub fn scored_completed_tools(&self, max: usize) -> Vec<CompletedToolCount> {
        const RECENCY_WEIGHT: f64 = 3.0;
        const DECAY_WINDOW_MS: u64 = 120_000; // 2 minutes

        let now = cache::now_epoch_ms();
        let mut scored: Vec<(CompletedToolCount, f64)> = self
            .completed_tool_counts
            .iter()
            .map(|(name, (count, last_at))| {
                let age_ms = now.saturating_sub(*last_at);
                let recency = if age_ms < DECAY_WINDOW_MS {
                    RECENCY_WEIGHT * (1.0 - age_ms as f64 / DECAY_WINDOW_MS as f64)
                } else {
                    0.0
                };
                let score = *count as f64 + recency;
                let failed = self.completed_tool_failures.get(name).copied().unwrap_or(0);
                (
                    CompletedToolCount {
                        name: name.clone(),
                        count: *count,
                        last_completed_at: Some(*last_at),
                        failed,
                    },
                    score,
                )
            })
            .collect();
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.name.cmp(&b.0.name))
        });
        scored.truncate(max);
        scored.into_iter().map(|(tool, _)| tool).collect()
    }

    pub fn upsert_agent(
        &mut self,
        id: String,
        description: String,
        agent_type: Option<String>,
        started_at: Option<u64>,
        model: Option<String>,
        message_id: Option<String>,
    ) {
        let (started_at, existing_model, existing_message_id, existing_agent_id) =
            if let Some(position) = self.active_agents.iter().position(|agent| agent.id == id) {
                let old = self.active_agents.remove(position);
                (old.started_at, old.model, old.message_id, old.agent_id)
            } else {
                let ts = started_at.or_else(|| {
                    Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    )
                });
                (ts, None, None, None)
            };
        self.active_agents.push(AgentSummary {
            id,
            description,
            agent_type,
            started_at,
            model: model.or(existing_model),
            completed_at: None,
            message_id: message_id.or(existing_message_id),
            agent_id: existing_agent_id,
            total_duration_ms: None,
            total_tokens: None,
            total_tool_use_count: None,
        });
    }

    pub fn remove_agent(&mut self, id: &str) {
        self.remove_agent_with_stats(id, None, None, None);
    }

    /// Move an agent from active to completed, optionally attaching completion stats.
    pub fn remove_agent_with_stats(
        &mut self,
        id: &str,
        total_duration_ms: Option<u64>,
        total_tokens: Option<u64>,
        total_tool_use_count: Option<u64>,
    ) {
        if let Some(pos) = self.active_agents.iter().position(|a| a.id == id) {
            let mut agent = self.active_agents.remove(pos);
            agent.completed_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );
            agent.total_duration_ms = total_duration_ms;
            agent.total_tokens = total_tokens;
            agent.total_tool_use_count = total_tool_use_count;
            self.completed_agents.push(agent);
            // Prune to max 10 completed agents
            if self.completed_agents.len() > 10 {
                let drain_count = self.completed_agents.len() - 10;
                self.completed_agents.drain(0..drain_count);
            }
        }
    }

    /// Drop an agent from the active list without recording it as
    /// completed. Used when an agent_progress event re-keys an existing
    /// active agent under a new id (e.g. progress assigns a runtime
    /// `agent_id` distinct from the original `tool_use_id`).
    pub fn discard_active_agent(&mut self, id: &str) {
        if let Some(pos) = self.active_agents.iter().position(|a| a.id == id) {
            self.active_agents.remove(pos);
        }
    }

    /// Attach a runtime `agentId` (from CC's async-agent `toolUseResult`)
    /// to the active agent stored under `tool_use_id`. Idempotent.
    pub fn bind_agent_id(&mut self, tool_use_id: &str, agent_id: &str) {
        if let Some(agent) = self.active_agents.iter_mut().find(|a| a.id == tool_use_id) {
            agent.agent_id = Some(agent_id.to_string());
        }
    }

    /// Create or refresh the sub-agent transcript-tail entry for
    /// `agent_id`, evicting the oldest entry by `last_event_ts` if the
    /// `MAX_SUBAGENT_TAILS` cap is exceeded.
    pub fn ensure_sub_agent(&mut self, agent_id: String, event_ts: Option<u64>) {
        let entry = self.sub_agents.entry(agent_id).or_default();
        if event_ts.is_some() {
            entry.last_event_ts = event_ts;
        }
        while self.sub_agents.len() > MAX_SUBAGENT_TAILS {
            let oldest = self
                .sub_agents
                .iter()
                .min_by_key(|(_, s)| s.last_event_ts.unwrap_or(0))
                .map(|(id, _)| id.clone());
            match oldest {
                Some(id) => {
                    self.sub_agents.remove(&id);
                }
                None => break,
            }
        }
    }

    pub fn set_todo(&mut self, todo: Option<TodoSummary>) {
        self.todo = todo;
    }

    // ── Agent linking methods ────────────────────────────────────────

    pub fn push_pending_task(
        &mut self,
        tool_use_id: String,
        description: String,
        agent_type: Option<String>,
        model: Option<String>,
        event_ts: Option<u64>,
        message_id: Option<String>,
    ) {
        self.pending_tasks.push(PendingTask {
            tool_use_id,
            description,
            agent_type,
            model,
            event_ts,
            message_id,
        });
    }

    /// FIFO pop from pending queue, store bidirectional link.
    pub fn link_agent_to_pending_task(&mut self, agent_id: &str) -> Option<PendingTask> {
        if self.pending_tasks.is_empty() {
            return None;
        }
        let pending = self.pending_tasks.remove(0);
        self.task_agent_links
            .insert(pending.tool_use_id.clone(), agent_id.to_string());
        Some(pending)
    }

    /// Lookup linked agentId for a tool_use_id.
    pub fn resolve_task_agent(&self, tool_use_id: &str) -> Option<String> {
        self.task_agent_links.get(tool_use_id).cloned()
    }

    /// Remove an unlinked pending task by its tool_use_id.
    pub fn drain_pending_task(&mut self, tool_use_id: &str) -> Option<PendingTask> {
        let pos = self
            .pending_tasks
            .iter()
            .position(|p| p.tool_use_id == tool_use_id)?;
        Some(self.pending_tasks.remove(pos))
    }

    /// Check if an agent was linked from an Agent tool_use.
    pub fn is_task_linked_agent(&self, agent_id: &str) -> bool {
        self.task_agent_links.values().any(|id| id == agent_id)
    }

    // ── Todo tracking methods ────────────────────────────────────────

    pub fn create_task_item(&mut self, subject: String, active_form: Option<String>) {
        self.task_counter += 1;
        let id = self.task_counter.to_string();
        self.task_items.insert(
            id,
            TaskItem {
                subject,
                status: "pending".to_string(),
                active_form,
                started_at: None,
            },
        );
        self.rebuild_todo_from_tasks();
    }

    pub fn update_task_item(&mut self, task_id: &str, status: &str) {
        if status == "deleted" {
            self.task_items.remove(task_id);
        } else if let Some(item) = self.task_items.get_mut(task_id) {
            if status == "in_progress" && item.status != "in_progress" {
                item.started_at = Some(cache::now_epoch_ms());
            }
            item.status = status.to_string();
        }
        self.rebuild_todo_from_tasks();
    }

    fn rebuild_todo_from_tasks(&mut self) {
        self.todo = build_todo_from_tasks(&self.task_items);
    }

    pub fn capped_tools(&self, max_lines: usize) -> Vec<ToolSummary> {
        if max_lines == 0 {
            return Vec::new();
        }
        let start = self.recent_tools.len().saturating_sub(max_lines);
        self.recent_tools[start..].to_vec()
    }

    /// Active agents first, then most recent completed, sliced to max_total.
    pub fn agents_for_display(&self, max_total: usize) -> Vec<AgentSummary> {
        if max_total == 0 {
            return Vec::new();
        }
        let mut result: Vec<AgentSummary> = Vec::new();

        // Active agents (running) — most recent N
        let active_start = self.active_agents.len().saturating_sub(max_total);
        result.extend_from_slice(&self.active_agents[active_start..]);

        // Fill remaining slots with completed agents (most recent first)
        let remaining = max_total.saturating_sub(result.len());
        if remaining > 0 {
            let mut completed: Vec<&AgentSummary> = self.completed_agents.iter().collect();
            completed.sort_by_key(|agent| std::cmp::Reverse(agent.completed_at));
            for agent in completed.into_iter().take(remaining) {
                result.push(agent.clone());
            }
        }

        result
    }

    /// Load state from a disk cache. Only restores env/git if within TTL.
    pub fn load_from_cache(&mut self, cache: SessionCache) {
        let now = cache::now_epoch_ms();

        // Transcript state
        self.last_transcript_offset = cache.transcript_offset;
        self.last_transcript_path = cache.transcript_path;
        self.active_tools = cache.active_tools;
        self.recent_tools = cache.recent_tools;
        self.active_agents = cache.active_agents;
        self.completed_agents = cache.completed_agents;
        self.completed_tool_counts = cache.completed_tool_counts;
        self.completed_tool_failures = cache.completed_tool_failures;
        self.todo = cache.todo;
        self.pending_tasks = cache.pending_tasks;
        self.task_agent_links = cache.task_agent_links;
        self.task_items = cache.task_items;
        self.task_counter = cache.task_counter;
        self.cached_line3 = cache.line3;
        self.last_output_tokens = cache.last_output_tokens;
        self.last_output_token_time_ms = cache.last_output_token_time_ms;
        self.output_speed_toks_per_sec = cache.output_speed_toks_per_sec;
        // Defensive trim: a future cap reduction must not over-restore here.
        let mut history = cache.ctx_history;
        while history.len() > MAX_CTX_HISTORY {
            history.pop_front();
        }
        self.ctx_history = history;
        let mut history = cache.cache_history;
        while history.len() > MAX_CACHE_HISTORY {
            history.pop_front();
        }
        self.cache_history = history;
        self.cache_read_total = cache.cache_read_total;
        self.last_usage_message_id = cache.last_usage_message_id;
        self.sub_agents = cache.sub_agents;
        self.compact_count = cache.compact_count;
        self.last_api_error_ms = cache.last_api_error_ms;

        // Env/Git only if within TTL
        if let Some(entry) = cache.env {
            if now.saturating_sub(entry.cached_at_ms) < CACHE_TTL_MS {
                self.cached_env = Some((entry.path, entry.snapshot));
            }
        }
        if let Some(entry) = cache.git {
            if now.saturating_sub(entry.cached_at_ms) < CACHE_TTL_MS {
                self.cached_git = Some((entry.path, entry.snapshot));
            }
        }
    }

    /// Export current state to a cache struct for disk persistence.
    pub fn to_cache(&self) -> SessionCache {
        let now = cache::now_epoch_ms();
        SessionCache {
            transcript_offset: self.last_transcript_offset,
            transcript_path: self.last_transcript_path.clone(),
            active_tools: self.active_tools.clone(),
            recent_tools: self.recent_tools.clone(),
            active_agents: self.active_agents.clone(),
            completed_agents: self.completed_agents.clone(),
            completed_tool_counts: self.completed_tool_counts.clone(),
            completed_tool_failures: self.completed_tool_failures.clone(),
            todo: self.todo.clone(),
            pending_tasks: self.pending_tasks.clone(),
            task_agent_links: self.task_agent_links.clone(),
            task_items: self.task_items.clone(),
            task_counter: self.task_counter,
            line3: self.cached_line3.clone(),
            last_output_tokens: self.last_output_tokens,
            last_output_token_time_ms: self.last_output_token_time_ms,
            output_speed_toks_per_sec: self.output_speed_toks_per_sec,
            ctx_history: self.ctx_history.clone(),
            cache_history: self.cache_history.clone(),
            cache_read_total: self.cache_read_total,
            last_usage_message_id: self.last_usage_message_id.clone(),
            sub_agents: self.sub_agents.clone(),
            compact_count: self.compact_count,
            last_api_error_ms: self.last_api_error_ms,
            env: self.cached_env.as_ref().map(|(path, snapshot)| CacheEntry {
                path: path.clone(),
                snapshot: snapshot.clone(),
                cached_at_ms: now,
            }),
            git: self.cached_git.as_ref().map(|(path, snapshot)| CacheEntry {
                path: path.clone(),
                snapshot: snapshot.clone(),
                cached_at_ms: now,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_output_speed_first_call_returns_none() {
        let mut state = SessionState::default();
        let result = state.update_output_speed(Some(100));
        assert!(result.is_none(), "first call has no previous data");
        assert_eq!(state.last_output_tokens, Some(100));
    }

    #[test]
    fn update_output_speed_computes_rate_within_window() {
        let now = cache::now_epoch_ms();
        let mut state = SessionState {
            last_output_tokens: Some(100),
            last_output_token_time_ms: Some(now.saturating_sub(500)),
            output_speed_toks_per_sec: None,
            ..SessionState::default()
        };

        let result = state.update_output_speed(Some(150));
        assert!(result.is_some(), "should compute speed");
        let speed = result.unwrap();
        assert!(
            speed > 50.0 && speed < 200.0,
            "speed {speed} should be ~100 tok/s"
        );
    }

    #[test]
    fn update_output_speed_keeps_last_beyond_2s() {
        let now = cache::now_epoch_ms();
        let mut state = SessionState {
            last_output_tokens: Some(100),
            last_output_token_time_ms: Some(now.saturating_sub(3000)),
            output_speed_toks_per_sec: Some(42.0),
            ..SessionState::default()
        };

        let result = state.update_output_speed(Some(200));
        assert_eq!(result, Some(42.0), "should keep last known speed");
    }

    #[test]
    fn update_speed_resets_on_transcript_change() {
        let mut state = SessionState {
            last_output_tokens: Some(100),
            last_output_token_time_ms: Some(1000),
            output_speed_toks_per_sec: Some(50.0),
            last_transcript_path: Some("/old/path".to_string()),
            ..SessionState::default()
        };

        state.reset_transcript_if_path_changed("/new/path");

        assert!(state.last_output_tokens.is_none());
        assert!(state.last_output_token_time_ms.is_none());
        assert!(state.output_speed_toks_per_sec.is_none());
    }

    #[test]
    fn update_output_speed_none_tokens_preserves_state() {
        let mut state = SessionState {
            last_output_tokens: Some(100),
            last_output_token_time_ms: Some(1000),
            output_speed_toks_per_sec: Some(50.0),
            ..SessionState::default()
        };

        let result = state.update_output_speed(None);
        // Bug #2 fix: None tokens should NOT corrupt state
        assert_eq!(result, Some(50.0), "should return last known speed");
        assert_eq!(
            state.last_output_tokens,
            Some(100),
            "should not overwrite tokens"
        );
        assert_eq!(
            state.last_output_token_time_ms,
            Some(1000),
            "should not overwrite time"
        );
    }

    #[test]
    fn scored_completed_tools_recent_low_count_beats_stale_moderate_count() {
        let mut state = SessionState::default();
        let now = cache::now_epoch_ms();

        // Moderate count tool, last used 3 minutes ago (beyond 2-min decay window)
        // Score: 3 + 0 = 3.0
        state
            .completed_tool_counts
            .insert("Grep".to_string(), (3, now.saturating_sub(180_000)));

        // Low count tool, used just now
        // Score: 1 + 3.0 = 4.0
        state
            .completed_tool_counts
            .insert("Skill".to_string(), (1, now));

        let top = state.scored_completed_tools(2);
        assert_eq!(top[0].name, "Skill", "recent tool should rank first");
        assert_eq!(top[1].name, "Grep", "stale tool should rank second");
    }

    #[test]
    fn scored_completed_tools_count_dominates_after_decay() {
        let mut state = SessionState::default();
        let now = cache::now_epoch_ms();

        // Both tools last used 3 minutes ago (beyond 2-min decay window)
        state
            .completed_tool_counts
            .insert("Read".to_string(), (15, now.saturating_sub(180_000)));
        state
            .completed_tool_counts
            .insert("Skill".to_string(), (1, now.saturating_sub(180_000)));

        let top = state.scored_completed_tools(2);
        assert_eq!(
            top[0].name, "Read",
            "higher count should win when both are stale"
        );
    }

    #[test]
    fn scored_completed_tools_tiebreak_alphabetical() {
        let mut state = SessionState::default();
        let now = cache::now_epoch_ms();

        // Same count, same recency
        state
            .completed_tool_counts
            .insert("Edit".to_string(), (5, now));
        state
            .completed_tool_counts
            .insert("Bash".to_string(), (5, now));

        let top = state.scored_completed_tools(2);
        assert_eq!(
            top[0].name, "Bash",
            "alphabetical tiebreak: Bash before Edit"
        );
        assert_eq!(top[1].name, "Edit");
    }

    // ── Cache-trend state ─────────────────────────────────────────────

    #[test]
    fn push_cache_sample_caps_at_max_cache_history() {
        let mut state = SessionState::default();
        for i in 0..(MAX_CACHE_HISTORY + 5) {
            state.push_cache_sample((i % 100) as u8, i as u64);
        }
        assert_eq!(state.cache_history.len(), MAX_CACHE_HISTORY);
        // 35 pushes into a 30-slot FIFO: samples 0-4 dropped, front is the
        // 6th sample (index 5).
        assert_eq!(state.cache_history.front(), Some(&(5u8, 5u64)));
    }

    #[test]
    fn reset_transcript_path_change_clears_cache_trend_state() {
        let mut state = SessionState {
            last_transcript_path: Some("/old/transcript.jsonl".to_string()),
            cache_read_total: 12_345,
            last_usage_message_id: Some("msg_old".to_string()),
            ..SessionState::default()
        };
        state.push_cache_sample(50, 1);
        state.push_cache_sample(60, 2);

        state.reset_transcript_if_path_changed("/new/transcript.jsonl");

        assert!(state.cache_history.is_empty());
        assert_eq!(state.cache_read_total, 0);
        assert_eq!(state.last_usage_message_id, None);
    }

    #[test]
    fn cache_trend_state_survives_cache_round_trip() {
        let mut state = SessionState {
            cache_read_total: 9_876,
            last_usage_message_id: Some("msg_rt".to_string()),
            ..SessionState::default()
        };
        state.push_cache_sample(42, 100);
        state.push_cache_sample(84, 200);

        // serde round-trip proves a process reload restores the fields.
        let serialized = serde_json::to_string(&state.to_cache()).expect("cache should serialize");
        let cache: SessionCache =
            serde_json::from_str(&serialized).expect("cache should deserialize");

        let mut fresh = SessionState::default();
        fresh.load_from_cache(cache);

        assert_eq!(
            fresh.cache_history,
            VecDeque::from(vec![(42u8, 100u64), (84u8, 200u64)])
        );
        assert_eq!(fresh.cache_read_total, 9_876);
        assert_eq!(fresh.last_usage_message_id, Some("msg_rt".to_string()));
    }

    #[test]
    fn load_from_cache_trims_oversized_cache_history() {
        let mut oversized = VecDeque::new();
        for i in 0..40u64 {
            oversized.push_back((i as u8, i));
        }
        let cache = SessionCache {
            cache_history: oversized,
            ..SessionCache::default()
        };

        let mut state = SessionState::default();
        state.load_from_cache(cache);

        assert_eq!(state.cache_history.len(), MAX_CACHE_HISTORY);
        // 40 entries trimmed to 30: oldest 10 dropped, front is entry 10.
        assert_eq!(state.cache_history.front(), Some(&(10u8, 10u64)));
    }

    #[test]
    fn session_cache_missing_cache_fields_deserializes_to_defaults() {
        let mut value =
            serde_json::to_value(SessionCache::default()).expect("cache should serialize");
        let map = value.as_object_mut().expect("cache should be an object");
        map.remove("cache_history");
        map.remove("cache_read_total");
        map.remove("last_usage_message_id");

        let cache: SessionCache =
            serde_json::from_value(value).expect("old-format cache should deserialize");

        assert!(cache.cache_history.is_empty());
        assert_eq!(cache.cache_read_total, 0);
        assert_eq!(cache.last_usage_message_id, None);
    }
}
