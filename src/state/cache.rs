use std::{
    collections::HashMap,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::{
    providers::{EnvSnapshot, GitSnapshot},
    types::{AgentSummary, Line3Metrics, PendingTask, TaskItem, TodoSummary, ToolSummary},
};

/// TTL for cached env/git snapshots (10 seconds).
pub const CACHE_TTL_MS: u64 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub path: String,
    pub snapshot: T,
    pub cached_at_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionCache {
    // Transcript state
    pub transcript_offset: u64,
    pub transcript_path: Option<String>,
    pub active_tools: Vec<ToolSummary>,
    pub active_agents: Vec<AgentSummary>,
    pub completed_agents: Vec<AgentSummary>,
    pub completed_tool_counts: HashMap<String, u32>,
    pub todo: Option<TodoSummary>,
    // Agent linking
    #[serde(default)]
    pub pending_tasks: Vec<PendingTask>,
    #[serde(default)]
    pub task_agent_links: HashMap<String, String>,
    // Todo tracking
    #[serde(default)]
    pub task_items: HashMap<String, TaskItem>,
    #[serde(default)]
    pub task_counter: u32,
    // L3 metrics (all-or-nothing fallback)
    pub line3: Option<Line3Metrics>,
    // Env/Git with timestamps
    pub env: Option<CacheEntry<EnvSnapshot>>,
    pub git: Option<CacheEntry<GitSnapshot>>,
}

/// Compute the cache file path for a session key.
pub fn cache_path(session_key: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    session_key.hash(&mut hasher);
    let hash = hasher.finish();
    std::env::temp_dir().join(format!("cc-pulseline-{hash:x}.json"))
}

/// Load a session cache from disk. Returns None on any error.
pub fn load_cache(session_key: &str) -> Option<SessionCache> {
    let path = cache_path(session_key);
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Save a session cache to disk with atomic write. Silently ignores errors.
pub fn save_cache(session_key: &str, cache: &SessionCache) {
    let path = cache_path(session_key);
    let contents = match serde_json::to_string(cache) {
        Ok(c) => c,
        Err(_) => return,
    };
    let tmp_path = path.with_extension("tmp");
    if fs::write(&tmp_path, contents).is_ok() {
        let _ = fs::rename(&tmp_path, &path);
    }
}

pub fn now_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
