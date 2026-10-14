pub mod config;
pub mod preview;
pub mod providers;
pub mod render;
pub mod state;
pub mod types;

use std::collections::HashMap;

use config::RenderConfig;
use providers::{
    EnvCollector, EnvSnapshot, FileSystemEnvCollector, FileTranscriptCollector, GitCollector,
    GitSnapshot, LocalGitCollector, PayloadActivityCollector, TranscriptCollector,
    TranscriptSnapshot,
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
        self.run_from_payload(&payload, config)
    }

    pub fn run_from_payload(
        &mut self,
        payload: &StdinPayload,
        config: RenderConfig,
    ) -> Result<Vec<String>, String> {
        let session_key = session_key(payload);
        let is_fresh = !self.sessions.contains_key(&session_key);
        let state = self.sessions.entry(session_key.clone()).or_default();

        // Load disk cache on first encounter of this session
        if is_fresh {
            if let Some(disk_cache) = cache::load_cache(&session_key) {
                state.load_from_cache(disk_cache);
            }
        }

        // A mod that hooks the engine's events hands the activity over on the
        // payload; only the classic statusline path parses the transcript.
        let transcript_snapshot = if payload.activity.is_some() {
            PayloadActivityCollector.collect_transcript(payload, state, &config)
        } else {
            self.transcript_collector
                .collect_transcript(payload, state, &config)
        };

        let project_path = payload
            .resolve_project_path()
            .unwrap_or_else(|| "unknown".to_string());
        let mut env_snapshot = collect_env_snapshot(&self.env_collector, state, &project_path);
        // A mod knows which of these actually loaded; the scan only knows
        // which files exist. Correct the counts it answered for.
        if let Some(over) = payload.env.as_ref() {
            apply_env_override(&mut env_snapshot, over);
        }
        let git_snapshot = collect_git_snapshot(&self.git_collector, state, &project_path);

        let mut frame =
            build_render_frame(payload, &env_snapshot, &git_snapshot, transcript_snapshot);

        // All-or-nothing L3 cache: if payload has no L3 data at all, use cached;
        // otherwise trust the payload entirely (no field-by-field merge).
        if frame.line3.has_data() {
            state.cached_line3 = Some(frame.line3.clone());
        } else if let Some(cached) = &state.cached_line3 {
            frame.line3 = cached.clone();
        }

        // Token speed: compute delta-based tok/s for output stream
        if config.show_speed {
            let usage = payload
                .context_window
                .as_ref()
                .and_then(|c| c.current_usage.as_ref());
            let output_tokens = usage.and_then(|u| u.output_tokens);
            frame.line3.output_speed_toks_per_sec = state.update_output_speed(output_tokens);
        }

        // Sparkline source. Dedup consecutive identical samples so an idle
        // statusline doesn't flatten the trail with one repeated value.
        if let Some(pct) = frame.line3.context_used_percentage {
            let sample = pct.min(100) as u8;
            let last = state.ctx_history.back().map(|(p, _)| *p);
            if last != Some(sample) {
                let now_ms = crate::state::cache::now_epoch_ms();
                state.push_ctx_sample(sample, now_ms);
            }
        }
        // Cache-trend source: per-tick hit-rate samples, deduped like ctx.
        if let Some(pct) = frame.line3.cache_hit_pct() {
            let sample = pct.round() as u8; // f64→u8 `as` saturates; helper clamps to 100
            let last = state.cache_history.back().map(|(p, _)| *p);
            if last != Some(sample) {
                let now_ms = crate::state::cache::now_epoch_ms();
                state.push_cache_sample(sample, now_ms);
            }
        }
        // CTX-history consumers: any layout whose effective `context_visual`
        // spec includes `sparkline` or `plot` (both braille trend widgets)
        // needs the history copy. Skip the copy otherwise — it's a tight
        // allocation hot path.
        let needs_ctx_history = config
            .effective_context_visual()
            .split('+')
            .any(|w| matches!(w.trim(), "sparkline" | "plot"));
        if needs_ctx_history {
            frame.ctx_history = state.ctx_history.iter().copied().collect();
        }
        // Cache-trend consumers (compact C-cell spark, ledger CACHE row) are
        // knob-gated; skip the copy otherwise — tight allocation hot path.
        if config.show_cache_trend {
            frame.cache_history = state.cache_history.iter().copied().collect();
            // The mod answers with its own running total; only the transcript
            // path accumulates into `SessionState`.
            frame.cache_read_total = payload
                .activity
                .as_ref()
                .and_then(|activity| activity.cache_read_total)
                .unwrap_or(state.cache_read_total);
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

/// Overwrites each count the caller answered, leaving the rest scanned.
fn apply_env_override(snapshot: &mut EnvSnapshot, over: &types::EnvOverride) {
    let fields: [(&mut u32, Option<u32>); 8] = [
        (&mut snapshot.claude_md_count, over.claude_md_count),
        (&mut snapshot.rules_count, over.rules_count),
        (&mut snapshot.hooks_count, over.hooks_count),
        (&mut snapshot.mcp_count, over.mcp_count),
        (&mut snapshot.memory_count, over.memory_count),
        (&mut snapshot.skills_count, over.skills_count),
        (&mut snapshot.plugins_count, over.plugins_count),
        (&mut snapshot.agents_md_count, over.agents_md_count),
    ];
    for (field, value) in fields {
        if let Some(value) = value {
            *field = value;
        }
    }
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
    collector: &LocalGitCollector,
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
    frame.line1.git_modified = git_snapshot.modified_count;
    frame.line1.git_added = git_snapshot.added_count;
    frame.line1.git_deleted = git_snapshot.deleted_count;
    frame.line1.git_untracked = git_snapshot.untracked_count;

    frame.line2.claude_md_count = env_snapshot.claude_md_count;
    frame.line2.agents_md_count = env_snapshot.agents_md_count;
    frame.line2.rules_count = env_snapshot.rules_count;
    frame.line2.memory_count = env_snapshot.memory_count;
    frame.line2.hooks_count = env_snapshot.hooks_count;
    frame.line2.mcp_count = env_snapshot.mcp_count;
    frame.line2.skills_count = env_snapshot.skills_count;
    frame.line2.plugins_count = env_snapshot.plugins_count;

    frame.tools = transcript_snapshot.tools;
    frame.completed_tools = transcript_snapshot.completed_counts;
    frame.completed_tool_total = transcript_snapshot.completed_total;
    frame.failed_tool_total = transcript_snapshot.failed_total;
    frame.agents = transcript_snapshot.agents;
    frame.todo = transcript_snapshot.todo;
    frame.compact_count = transcript_snapshot.compact_count;
    // Apply 30s TTL: only surface the api_error badge if it occurred recently.
    frame.last_api_error_ms = transcript_snapshot.last_api_error_ms.filter(|&ts| {
        let now = crate::state::cache::now_epoch_ms();
        now.saturating_sub(ts) < 30_000
    });

    frame
}

/// The disk cache's key: `session_id|transcript_path|project_path`.
///
/// The transcript path is in it, and a mod sends no `transcript_path` — so the
/// band and the statusline key *different* cache files even inside one
/// session, and neither can roll back the other's transcript offset or
/// accumulated state. Checked, not assumed: one invocation of each with the
/// same session id and project writes two files.
///
/// A caller that sent both `activity` and a `transcript_path` would share one
/// file with the statusline. Nothing does today; it would need its own
/// namespace if anything did.
fn session_key(payload: &StdinPayload) -> String {
    format!(
        "{}|{}|{}",
        payload.session_id.as_deref().unwrap_or(""),
        payload.transcript_path.as_deref().unwrap_or(""),
        payload.resolve_project_path().as_deref().unwrap_or("")
    )
}
