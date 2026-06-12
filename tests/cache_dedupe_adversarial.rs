//! Adversarial probes for the cache-read dedupe logic (criterion A3).
//!
//! Exercises the four prescribed cases — streaming-chunk repeats, distinct
//! calls with identical usage, process restart mid-transcript, and
//! truncation/reset — plus the session-resume replay pattern observed in
//! real Claude Code transcripts (CC re-appends the full conversation
//! history, byte-near-identical lines with the same `message.id`/`uuid`,
//! into the SAME transcript file on resume).

use std::fs;

use cc_pulseline::{
    config::RenderConfig,
    providers::{FileTranscriptCollector, TranscriptCollector},
    state::{cache, SessionState},
    types::StdinPayload,
    PulseLineRunner,
};
use serde_json::json;
use tempfile::TempDir;

fn append_line(path: &std::path::Path, line: &str) {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("transcript file should open");
    writeln!(file, "{line}").expect("line should append");
}

fn payload(
    workspace: &TempDir,
    transcript_path: &std::path::Path,
    session_id: &str,
) -> StdinPayload {
    let raw = json!({
        "session_id": session_id,
        "transcript_path": transcript_path,
        "workspace": {"current_dir": workspace.path()},
    })
    .to_string();
    serde_json::from_str::<StdinPayload>(&raw).expect("payload should deserialize")
}

fn test_config() -> RenderConfig {
    RenderConfig {
        transcript_poll_throttle_ms: 0,
        color_enabled: false,
        ..Default::default()
    }
}

/// One assistant usage line in real transcript shape. `uuid` is the
/// transcript-event uuid (distinct per streamed chunk line); `message_id`
/// is the Anthropic API response id (shared across chunks of one call).
fn usage_line(message_id: &str, cache_read: u64, uuid: &str) -> String {
    json!({
        "type": "assistant",
        "isSidechain": false,
        "message": {
            "id": message_id,
            "role": "assistant",
            "content": [{"type": "text", "text": "x"}],
            "usage": {
                "input_tokens": 100,
                "cache_creation_input_tokens": 200,
                "cache_read_input_tokens": cache_read,
                "output_tokens": 5
            }
        },
        "requestId": format!("req_{message_id}"),
        "uuid": uuid,
        "timestamp": "2026-06-12T00:00:00.000Z"
    })
    .to_string()
}

/// A user tool_result line — carries no `message.id` (real CC shape).
fn user_tool_result_line(uuid: &str) -> String {
    json!({
        "type": "user",
        "isSidechain": false,
        "message": {
            "role": "user",
            "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": "ok"}]
        },
        "uuid": uuid,
        "timestamp": "2026-06-12T00:00:01.000Z"
    })
    .to_string()
}

fn session_key(workspace: &TempDir, transcript: &std::path::Path, session_id: &str) -> String {
    format!(
        "{session_id}|{}|{}",
        transcript.to_string_lossy(),
        workspace.path().to_string_lossy()
    )
}

// ---------------------------------------------------------------------------
// Case 1: same API call's usage repeated across streaming chunk lines.
// ---------------------------------------------------------------------------

#[test]
fn streaming_chunks_count_once_even_with_interposed_non_id_lines() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("chunks.jsonl");
    let payload = payload(&workspace, &transcript, "adv-chunks");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    // Three chunk lines of ONE API call, with a user tool_result line (no
    // message.id) interposed — dedupe must survive the gap.
    append_line(&transcript, &usage_line("msg_A", 1234, "u1"));
    append_line(&transcript, &usage_line("msg_A", 1234, "u2"));
    append_line(&transcript, &user_tool_result_line("u3"));
    append_line(&transcript, &usage_line("msg_A", 1234, "u4"));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(state.cache_read_total, 1234, "one call = one count");

    // A late chunk of the SAME call arriving in a later poll.
    append_line(&transcript, &usage_line("msg_A", 1234, "u5"));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(state.cache_read_total, 1234, "late chunk must not recount");
}

// ---------------------------------------------------------------------------
// Case 2: two distinct API calls with IDENTICAL usage numbers.
// ---------------------------------------------------------------------------

#[test]
fn distinct_calls_with_identical_usage_each_count() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("identical.jsonl");
    let payload = payload(&workspace, &transcript, "adv-identical");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    // Adjacent calls, byte-identical usage — only message.id differs.
    append_line(&transcript, &usage_line("msg_A", 4242, "u1"));
    append_line(&transcript, &usage_line("msg_B", 4242, "u2"));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(
        state.cache_read_total, 8484,
        "identical usage is not a dupe"
    );

    // Third identical-usage call in a separate poll.
    append_line(&transcript, &usage_line("msg_C", 4242, "u3"));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(state.cache_read_total, 12726);
}

// ---------------------------------------------------------------------------
// Case 3: process restart mid-transcript (disk cache reload + offset resume).
// ---------------------------------------------------------------------------

#[test]
fn process_restart_mid_stream_does_not_recount_partial_message() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("restart.jsonl");
    let session_id = "adv-restart-mid-stream";
    let payload_json = json!({
        "session_id": session_id,
        "transcript_path": transcript,
        "workspace": {"current_dir": workspace.path()},
    })
    .to_string();
    let key = session_key(&workspace, &transcript, session_id);

    // Process 1 sees only chunk 1 of msg_A, persists, dies.
    append_line(&transcript, &usage_line("msg_A", 1000, "u1"));
    {
        let mut runner1 = PulseLineRunner::default();
        runner1
            .run_from_str(&payload_json, test_config())
            .expect("process 1 renders");
    }
    let disk = cache::load_cache(&key).expect("disk cache after process 1");
    assert_eq!(disk.cache_read_total, 1000);
    assert_eq!(disk.last_usage_message_id.as_deref(), Some("msg_A"));

    // While no process runs: remaining chunks of msg_A + a new call msg_B.
    append_line(&transcript, &usage_line("msg_A", 1000, "u2"));
    append_line(&transcript, &usage_line("msg_A", 1000, "u3"));
    append_line(&transcript, &usage_line("msg_B", 500, "u4"));

    // Fresh process resumes from the persisted offset + dedupe id.
    let mut runner2 = PulseLineRunner::default();
    runner2
        .run_from_str(&payload_json, test_config())
        .expect("process 2 renders");
    let disk = cache::load_cache(&key).expect("disk cache after process 2");
    assert_eq!(
        disk.cache_read_total, 1500,
        "restart mid-message must not recount msg_A chunks"
    );
}

#[test]
fn restart_with_lost_cache_recounts_from_zero_exactly() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("lostcache.jsonl");
    let session_id = "adv-restart-lost-cache";
    let payload_json = json!({
        "session_id": session_id,
        "transcript_path": transcript,
        "workspace": {"current_dir": workspace.path()},
    })
    .to_string();
    let key = session_key(&workspace, &transcript, session_id);

    append_line(&transcript, &usage_line("msg_A", 1000, "u1"));
    append_line(&transcript, &usage_line("msg_B", 500, "u2"));
    {
        let mut runner1 = PulseLineRunner::default();
        runner1
            .run_from_str(&payload_json, test_config())
            .expect("process 1 renders");
    }
    assert_eq!(
        cache::load_cache(&key)
            .expect("cache exists")
            .cache_read_total,
        1500
    );

    // Temp cache evicted (e.g. OS cleaned /tmp) → full re-read from 0 must
    // land on the same exact total, not accumulate on top of stale state.
    fs::remove_file(cache::cache_path(&key)).expect("cache file removed");
    let mut runner2 = PulseLineRunner::default();
    runner2
        .run_from_str(&payload_json, test_config())
        .expect("process 2 renders");
    assert_eq!(
        cache::load_cache(&key)
            .expect("cache rewritten")
            .cache_read_total,
        1500,
        "full re-read recomputes the exact total"
    );
}

// ---------------------------------------------------------------------------
// Case 4: transcript truncation/reset — including across a process restart.
// ---------------------------------------------------------------------------

#[test]
fn truncation_across_process_restart_resets_exactly() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("trunc.jsonl");
    let session_id = "adv-trunc-restart";
    let payload_json = json!({
        "session_id": session_id,
        "transcript_path": transcript,
        "workspace": {"current_dir": workspace.path()},
    })
    .to_string();
    let key = session_key(&workspace, &transcript, session_id);

    append_line(&transcript, &usage_line("msg_A", 1000, "u1"));
    append_line(&transcript, &usage_line("msg_B", 500, "u2"));
    {
        let mut runner1 = PulseLineRunner::default();
        runner1
            .run_from_str(&payload_json, test_config())
            .expect("process 1 renders");
    }

    // File replaced with SHORTER content while no process runs. The new
    // file reuses msg_A: the truncation reset must also clear the dedupe
    // id, or msg_A's usage would be skipped (undercount).
    fs::write(&transcript, format!("{}\n", usage_line("msg_A", 700, "u9")))
        .expect("transcript rewritten");

    let mut runner2 = PulseLineRunner::default();
    runner2
        .run_from_str(&payload_json, test_config())
        .expect("process 2 renders");
    assert_eq!(
        cache::load_cache(&key)
            .expect("cache after truncation")
            .cache_read_total,
        700,
        "truncation resets then re-accumulates only new content"
    );
}

// ---------------------------------------------------------------------------
// KNOWN BUG repro: session-resume history replay double-counts.
//
// Real CC behavior (observed in ~/.claude/projects/*.jsonl): resuming a
// session re-appends the ENTIRE conversation history to the SAME transcript
// file — byte-near-identical lines with the same `message.id`, `uuid`,
// `requestId`, timestamp, and usage. The single-remembered-id dedupe
// (`last_usage_message_id`) only suppresses contiguous repeats, so every
// replayed usage event is re-accumulated. Measured on a real transcript
// with two resume replays: algorithm total 1,735,405,509 vs ground truth
// 899,558,894 (1.93x inflation). Un-ignore once dedupe is replay-proof
// (e.g. keyed on seen message ids with a FIFO cap, or replay-marker reset).
// ---------------------------------------------------------------------------

#[test]
#[ignore = "KNOWN BUG: session-resume history replay double-counts cache_read_total"]
fn session_resume_history_replay_must_not_recount() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("resume.jsonl");
    let payload = payload(&workspace, &transcript, "adv-resume-replay");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    // Live session: two API calls.
    append_line(&transcript, &usage_line("msg_A", 1000, "u1"));
    append_line(&transcript, &usage_line("msg_B", 500, "u2"));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(state.cache_read_total, 1500);

    // Session resume: CC appends session-restart metadata, then replays the
    // full history with IDENTICAL message ids / uuids / usage. The file
    // only grows, so the truncation reset does not fire.
    append_line(
        &transcript,
        &json!({"type": "last-prompt", "lastPrompt": "continue"}).to_string(),
    );
    append_line(&transcript, &usage_line("msg_A", 1000, "u1"));
    append_line(&transcript, &usage_line("msg_B", 500, "u2"));
    collector.collect_transcript(&payload, &mut state, &config);

    assert_eq!(
        state.cache_read_total, 1500,
        "replayed history lines are the SAME API calls and must not recount \
         (actual behavior today: 3000 — every resume roughly doubles the total)"
    );
}
