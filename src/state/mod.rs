use std::{collections::HashMap, time::Instant};

use crate::{
    providers::{EnvSnapshot, GitSnapshot},
    types::{AgentSummary, TodoSummary, ToolSummary},
};

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
    pub todo: Option<TodoSummary>,
    pub cached_env: Option<(String, EnvSnapshot)>,
    pub cached_git: Option<(String, GitSnapshot)>,
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
            self.todo = None;
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

    pub fn upsert_tool(&mut self, id: String, text: String) {
        if let Some(position) = self.active_tools.iter().position(|tool| tool.id == id) {
            self.active_tools.remove(position);
        }
        self.active_tools.push(ToolSummary { id, text });
    }

    pub fn remove_tool(&mut self, id: &str) {
        self.active_tools.retain(|tool| tool.id != id);
    }

    pub fn upsert_agent(&mut self, id: String, text: String) {
        if let Some(position) = self.active_agents.iter().position(|agent| agent.id == id) {
            self.active_agents.remove(position);
        }
        self.active_agents.push(AgentSummary { id, text });
    }

    pub fn remove_agent(&mut self, id: &str) {
        self.active_agents.retain(|agent| agent.id != id);
    }

    pub fn set_todo(&mut self, todo: Option<TodoSummary>) {
        self.todo = todo;
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
}

#[derive(Debug, Default)]
pub struct RunnerState {
    pub sessions: HashMap<String, SessionState>,
}
