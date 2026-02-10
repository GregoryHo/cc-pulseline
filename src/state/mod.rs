pub mod cache;

use std::{
    collections::HashMap,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    providers::{EnvSnapshot, GitSnapshot},
    types::{
        AgentSummary, CompletedToolCount, Line3Metrics, PendingTask, TaskItem, TodoSummary,
        ToolSummary,
    },
};
use cache::{CacheEntry, SessionCache, CACHE_TTL_MS};

#[derive(Debug, Clone, Default)]
pub struct RenderCacheEntry {
    pub key: u64,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionState {
    pub last_transcript_offset: u64,
    pub last_transcript_path: Option<String>,
    pub last_transcript_poll: Option<Instant>,
    pub active_tools: Vec<ToolSummary>,
    pub active_agents: Vec<AgentSummary>,
    pub completed_agents: Vec<AgentSummary>,
    pub completed_tool_counts: HashMap<String, u32>,
    pub todo: Option<TodoSummary>,
    // Agent linking: Task tool_use → agent_progress ID linking
    pub pending_tasks: Vec<PendingTask>,
    pub task_agent_links: HashMap<String, String>, // tool_use_id → agentId
    // Todo tracking: TaskCreate/TaskUpdate stateful accumulation
    pub task_items: HashMap<String, TaskItem>,
    pub task_counter: u32,
    pub cached_env: Option<(String, EnvSnapshot)>,
    pub cached_git: Option<(String, GitSnapshot)>,
    pub cached_line3: Option<Line3Metrics>,
    pub render_cache: Option<RenderCacheEntry>,
}

impl SessionState {
    pub fn reset_transcript_if_path_changed(&mut self, transcript_path: &str) {
        if self.last_transcript_path.as_deref() != Some(transcript_path) {
            self.last_transcript_path = Some(transcript_path.to_string());
            self.last_transcript_offset = 0;
            self.last_transcript_poll = None;
            self.active_tools.clear();
            self.active_agents.clear();
            self.completed_agents.clear();
            self.completed_tool_counts.clear();
            self.todo = None;
            self.pending_tasks.clear();
            self.task_agent_links.clear();
            self.task_items.clear();
            self.task_counter = 0;
            self.cached_line3 = None;
        }
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
        self.active_tools.push(ToolSummary { id, name, target });
    }

    pub fn remove_tool(&mut self, id: &str) {
        if let Some(tool) = self.active_tools.iter().find(|t| t.id == id) {
            self.record_tool_completion(&tool.name.clone());
        }
        self.active_tools.retain(|tool| tool.id != id);
    }

    pub fn record_tool_completion(&mut self, name: &str) {
        *self.completed_tool_counts.entry(name.to_string()).or_insert(0) += 1;
    }

    pub fn top_completed_tools(&self, max: usize) -> Vec<CompletedToolCount> {
        let mut counts: Vec<CompletedToolCount> = self
            .completed_tool_counts
            .iter()
            .map(|(name, count)| CompletedToolCount {
                name: name.clone(),
                count: *count,
            })
            .collect();
        counts.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
        counts.truncate(max);
        counts
    }

    pub fn upsert_agent(
        &mut self,
        id: String,
        description: String,
        agent_type: Option<String>,
        started_at: Option<u64>,
        model: Option<String>,
    ) {
        let (started_at, existing_model) = if let Some(position) =
            self.active_agents.iter().position(|agent| agent.id == id)
        {
            let old = self.active_agents.remove(position);
            (old.started_at, old.model)
        } else {
            let ts = started_at.or_else(|| {
                Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                )
            });
            (ts, None)
        };
        self.active_agents.push(AgentSummary {
            id,
            description,
            agent_type,
            started_at,
            model: model.or(existing_model),
            completed_at: None,
        });
    }

    pub fn remove_agent(&mut self, id: &str) {
        if let Some(pos) = self.active_agents.iter().position(|a| a.id == id) {
            let mut agent = self.active_agents.remove(pos);
            agent.completed_at = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            );
            self.completed_agents.push(agent);
            // Prune to max 10 completed agents
            if self.completed_agents.len() > 10 {
                let drain_count = self.completed_agents.len() - 10;
                self.completed_agents.drain(0..drain_count);
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
    ) {
        self.pending_tasks.push(PendingTask {
            tool_use_id,
            description,
            agent_type,
            model,
            event_ts,
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

    /// Check if an agent was linked from a Task tool_use.
    pub fn is_task_linked_agent(&self, agent_id: &str) -> bool {
        self.task_agent_links.values().any(|id| id == agent_id)
    }

    // ── Todo tracking methods ────────────────────────────────────────

    pub fn create_task_item(&mut self, subject: String) {
        self.task_counter += 1;
        let id = self.task_counter.to_string();
        self.task_items.insert(
            id,
            TaskItem {
                subject,
                status: "pending".to_string(),
            },
        );
        self.rebuild_todo_from_tasks();
    }

    pub fn update_task_item(&mut self, task_id: &str, status: &str) {
        if status == "deleted" {
            self.task_items.remove(task_id);
        } else if let Some(item) = self.task_items.get_mut(task_id) {
            item.status = status.to_string();
        }
        self.rebuild_todo_from_tasks();
    }

    fn rebuild_todo_from_tasks(&mut self) {
        if self.task_items.is_empty() {
            self.todo = None;
            return;
        }
        let total = self.task_items.len();
        let completed = self
            .task_items
            .values()
            .filter(|item| item.status == "completed")
            .count();
        let pending = total.saturating_sub(completed);
        if pending == 0 {
            self.todo = None;
            return;
        }
        self.todo = Some(TodoSummary {
            text: format!("{completed}/{total} done, {pending} pending"),
            pending,
            completed,
            total,
        });
    }

    pub fn capped_tools(&self, max_lines: usize) -> Vec<ToolSummary> {
        if max_lines == 0 {
            return Vec::new();
        }
        let start = self.active_tools.len().saturating_sub(max_lines);
        self.active_tools[start..].to_vec()
    }

    pub fn capped_agents(&self, max_lines: usize) -> Vec<AgentSummary> {
        if max_lines == 0 {
            return Vec::new();
        }
        let start = self.active_agents.len().saturating_sub(max_lines);
        self.active_agents[start..].to_vec()
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
            completed.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
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
        self.active_agents = cache.active_agents;
        self.completed_agents = cache.completed_agents;
        self.completed_tool_counts = cache.completed_tool_counts;
        self.todo = cache.todo;
        self.pending_tasks = cache.pending_tasks;
        self.task_agent_links = cache.task_agent_links;
        self.task_items = cache.task_items;
        self.task_counter = cache.task_counter;
        self.cached_line3 = cache.line3;

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
            active_agents: self.active_agents.clone(),
            completed_agents: self.completed_agents.clone(),
            completed_tool_counts: self.completed_tool_counts.clone(),
            todo: self.todo.clone(),
            pending_tasks: self.pending_tasks.clone(),
            task_agent_links: self.task_agent_links.clone(),
            task_items: self.task_items.clone(),
            task_counter: self.task_counter,
            line3: self.cached_line3.clone(),
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

#[derive(Debug, Default)]
pub struct RunnerState {
    pub sessions: HashMap<String, SessionState>,
}
