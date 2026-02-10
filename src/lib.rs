pub mod config;
pub mod providers;
pub mod render;
pub mod state;
pub mod types;

use std::collections::HashMap;

use config::RenderConfig;
use providers::{
    EnvCollector, EnvSnapshot, FileSystemEnvCollector, FileTranscriptCollector, GitCollector,
    GitSnapshot, LocalGitCollector, TranscriptCollector, TranscriptSnapshot,
};
use state::cache;
use state::SessionState;
use types::{RenderFrame, StdinPayload};

#[derive(Debug, Default)]
pub struct PulseLineRunner {
    sessions: HashMap<String, SessionState>,
    env_collector: FileSystemEnvCollector,
    git_collector: LocalGitCollector,
    transcript_collector: FileTranscriptCollector,
}

impl PulseLineRunner {
    pub fn with_user_home(mut self, home: std::path::PathBuf) -> Self {
        self.env_collector.user_home_override = Some(home);
        self
    }

    pub fn run_from_str(
        &mut self,
        input: &str,
        config: RenderConfig,
    ) -> Result<Vec<String>, String> {
        let payload: StdinPayload =
            serde_json::from_str(input).map_err(|error| format!("invalid stdin JSON: {error}"))?;

        let session_key = session_key(&payload);
        let is_fresh = !self.sessions.contains_key(&session_key);
        let state = self.sessions.entry(session_key.clone()).or_default();

        // Load disk cache on first encounter of this session
        if is_fresh {
            if let Some(disk_cache) = cache::load_cache(&session_key) {
                state.load_from_cache(disk_cache);
            }
        }

        let transcript_snapshot = self
            .transcript_collector
            .collect_transcript(&payload, state, &config);

        let project_path = payload
            .resolve_project_path()
            .unwrap_or_else(|| "unknown".to_string());
        let env_snapshot = collect_env_snapshot(&self.env_collector, state, &project_path);
        let git_snapshot = collect_git_snapshot(self.git_collector, state, &project_path);

        let mut frame =
            build_render_frame(&payload, &env_snapshot, &git_snapshot, transcript_snapshot);

        // All-or-nothing L3 cache: if payload has no L3 data at all, use cached;
        // otherwise trust the payload entirely (no field-by-field merge).
        if frame.line3.has_data() {
            state.cached_line3 = Some(frame.line3.clone());
        } else if let Some(cached) = &state.cached_line3 {
            frame.line3 = cached.clone();
        }

        let lines = render::layout::render_frame(&frame, &config);

        // Save cache to disk
        cache::save_cache(&session_key, &state.to_cache());

        Ok(lines)
    }
}

pub fn run_from_str(input: &str, config: RenderConfig) -> Result<Vec<String>, String> {
    PulseLineRunner::default().run_from_str(input, config)
}

fn collect_env_snapshot(
    collector: &FileSystemEnvCollector,
    state: &mut SessionState,
    project_path: &str,
) -> EnvSnapshot {
    if let Some(snapshot) = state.cached_env_for(project_path) {
        return snapshot;
    }

    let snapshot = if project_path == "unknown" {
        EnvSnapshot::default()
    } else {
        collector.collect_env(project_path)
    };

    state.set_cached_env(project_path.to_string(), snapshot.clone());
    snapshot
}

fn collect_git_snapshot(
    collector: LocalGitCollector,
    state: &mut SessionState,
    project_path: &str,
) -> GitSnapshot {
    if let Some(snapshot) = state.cached_git_for(project_path) {
        return snapshot;
    }

    let snapshot = if project_path == "unknown" {
        GitSnapshot::default()
    } else {
        collector.collect_git(project_path)
    };

    state.set_cached_git(project_path.to_string(), snapshot.clone());
    snapshot
}

fn build_render_frame(
    payload: &StdinPayload,
    env_snapshot: &EnvSnapshot,
    git_snapshot: &GitSnapshot,
    transcript_snapshot: TranscriptSnapshot,
) -> RenderFrame {
    let mut frame = RenderFrame::from_payload(payload);

    frame.line1.git_branch = git_snapshot.branch.clone();
    frame.line1.git_dirty = git_snapshot.dirty;
    frame.line1.git_ahead = git_snapshot.ahead;
    frame.line1.git_behind = git_snapshot.behind;

    frame.line2.claude_md_count = env_snapshot.claude_md_count;
    frame.line2.rules_count = env_snapshot.rules_count;
    frame.line2.hooks_count = env_snapshot.hooks_count;
    frame.line2.mcp_count = env_snapshot.mcp_count;
    frame.line2.skills_count = env_snapshot.skills_count;

    frame.tools = transcript_snapshot.tools;
    frame.completed_tools = transcript_snapshot.completed_counts;
    frame.agents = transcript_snapshot.agents;
    frame.todo = transcript_snapshot.todo;

    frame
}

fn session_key(payload: &StdinPayload) -> String {
    format!(
        "{}|{}|{}",
        payload.session_id.as_deref().unwrap_or(""),
        payload.transcript_path.as_deref().unwrap_or(""),
        payload.resolve_project_path().as_deref().unwrap_or("")
    )
}

