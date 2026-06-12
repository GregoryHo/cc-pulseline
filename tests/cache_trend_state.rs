//! Cache-trend session state: cumulative cache-read accumulation from
//! transcript usage events (deduped per API call by `message.id`) and
//! the cache hit-rate history buffer lifecycle.

use std::{
    fs::{self, OpenOptions},
    io::Write,
};

use cc_pulseline::{
    config::RenderConfig,
    providers::{FileTranscriptCollector, TranscriptCollector},
    state::SessionState,
    types::StdinPayload,
    PulseLineRunner,
};
use serde_json::json;
use tempfile::TempDir;

fn append_line(path: &std::path::Path, line: &str) {
    let mut file = OpenOptions::new()
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
        ..Default::default()
    }
}

/// One assistant usage line in real transcript shape. `ts_secs` is the
/// line timestamp as seconds past a fixed base — distinct API calls carry
/// increasing timestamps (the `replay_guard_ts_ms` replay guard skips
/// anything at or below the high-water mark).
fn usage_line(message_id: &str, cache_read: u64, uuid: &str, ts_secs: u64) -> String {
    json!({
        "type": "assistant",
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
        "requestId": format!("req_{uuid}"),
        "uuid": uuid,
        "timestamp": format!("2026-06-12T00:{:02}:{:02}.000Z", ts_secs / 60, ts_secs % 60)
    })
    .to_string()
}

#[test]
fn usage_events_accumulate_and_dedupe_by_message_id() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("usage.jsonl");
    let payload = payload(&workspace, &transcript, "cache-trend-accumulate");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    append_line(&transcript, &usage_line("msg_1", 1000, "u1", 0));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(
        state.cache_read_total, 1000,
        "first usage event accumulates"
    );

    // Same id + identical usage (streaming chunk) must NOT double-count.
    append_line(&transcript, &usage_line("msg_1", 1000, "u2", 1));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(state.cache_read_total, 1000, "same message.id is deduped");

    append_line(&transcript, &usage_line("msg_2", 500, "u3", 5));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(state.cache_read_total, 1500, "new message.id increments");
}

#[test]
fn dedupe_survives_split_across_polls() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("split.jsonl");
    let payload = payload(&workspace, &transcript, "cache-trend-split");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    // Chunk 1 of msg_1 lands in the first poll...
    append_line(&transcript, &usage_line("msg_1", 1000, "u1", 0));
    collector.collect_transcript(&payload, &mut state, &config);
    // ...chunk 2 of the same API call lands in the next poll.
    append_line(&transcript, &usage_line("msg_1", 1000, "u2", 1));
    collector.collect_transcript(&payload, &mut state, &config);

    assert_eq!(
        state.cache_read_total, 1000,
        "one logical API call split across polls must count once"
    );
}

#[test]
fn compact_boundary_clears_history_but_not_cumulative() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("compact.jsonl");
    let payload = payload(&workspace, &transcript, "cache-trend-compact");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    append_line(&transcript, &usage_line("msg_1", 1000, "u1", 0));
    collector.collect_transcript(&payload, &mut state, &config);
    state.push_cache_sample(80, 1);
    state.push_cache_sample(90, 2);

    append_line(
        &transcript,
        &json!({
            "type": "system",
            "subtype": "compact_boundary",
            "timestamp": "2026-06-12T00:01:00.000Z"
        })
        .to_string(),
    );
    collector.collect_transcript(&payload, &mut state, &config);

    assert!(
        state.cache_history.is_empty(),
        "compaction clears the trend window"
    );
    assert_eq!(
        state.cache_read_total, 1000,
        "cumulative read total survives compaction"
    );
    assert_eq!(state.compact_count, 1);
}

#[test]
fn transcript_path_change_clears_cumulative_and_history() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript_a = workspace.path().join("a.jsonl");
    let transcript_b = workspace.path().join("b.jsonl");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    append_line(&transcript_a, &usage_line("msg_1", 1000, "u1", 0));
    let payload_a = payload(&workspace, &transcript_a, "cache-trend-path");
    collector.collect_transcript(&payload_a, &mut state, &config);
    state.push_cache_sample(75, 1);
    assert_eq!(state.cache_read_total, 1000);

    append_line(&transcript_b, &json!({"type": "user"}).to_string());
    let payload_b = payload(&workspace, &transcript_b, "cache-trend-path");
    collector.collect_transcript(&payload_b, &mut state, &config);

    assert_eq!(state.cache_read_total, 0, "path change resets the total");
    assert!(state.cache_history.is_empty(), "path change clears history");
    assert_eq!(state.last_usage_message_id, None);
}

#[test]
fn transcript_truncation_resets_cache_trend() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("trunc.jsonl");
    let payload = payload(&workspace, &transcript, "cache-trend-trunc");
    let config = test_config();
    let collector = FileTranscriptCollector;
    let mut state = SessionState::default();

    append_line(&transcript, &usage_line("msg_1", 1000, "u1", 0));
    append_line(&transcript, &usage_line("msg_2", 500, "u2", 5));
    collector.collect_transcript(&payload, &mut state, &config);
    assert_eq!(state.cache_read_total, 1500);

    // Replace the file with SHORTER content: truncation must reset the
    // accumulator before re-reading from offset 0 (not old + 700).
    fs::write(
        &transcript,
        format!("{}\n", usage_line("msg_9", 700, "u9", 0)),
    )
    .expect("transcript should rewrite");
    collector.collect_transcript(&payload, &mut state, &config);

    assert_eq!(
        state.cache_read_total, 700,
        "truncation resets, then re-accumulates only the new content"
    );
}

#[test]
fn runner_persists_cache_trend_to_disk_cache() {
    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("runner.jsonl");
    let session_id = "cache-trend-runner-persist-2026";

    let payload_with_usage = |cache_read: u64| {
        json!({
            "session_id": session_id,
            "transcript_path": transcript,
            "workspace": {"current_dir": workspace.path()},
            "context_window": {
                "context_window_size": 200000,
                "used_percentage": 5,
                "current_usage": {
                    "input_tokens": 1000,
                    "cache_creation_input_tokens": 500,
                    "cache_read_input_tokens": cache_read,
                    "output_tokens": 10
                }
            }
        })
        .to_string()
    };

    let mut runner = PulseLineRunner::default();
    let config = test_config();

    // Two identical runs: hit pct 8500/10000 = 85% both times → one sample.
    runner
        .run_from_str(&payload_with_usage(8500), config.clone())
        .expect("render should succeed");
    runner
        .run_from_str(&payload_with_usage(8500), config.clone())
        .expect("render should succeed");

    let project = workspace.path().to_string_lossy().to_string();
    let key = format!("{session_id}|{}|{project}", transcript.to_string_lossy());
    let cache =
        cc_pulseline::state::cache::load_cache(&key).expect("disk cache should exist after a run");
    assert_eq!(
        cache.cache_history.len(),
        1,
        "consecutive identical hit-rate samples dedupe to one entry"
    );
    assert_eq!(cache.cache_history[0].0, 85);

    // Changed usage: 2000/3500 ≈ 57% → second sample.
    runner
        .run_from_str(&payload_with_usage(2000), config)
        .expect("render should succeed");
    let cache =
        cc_pulseline::state::cache::load_cache(&key).expect("disk cache should exist after a run");
    assert_eq!(cache.cache_history.len(), 2, "changed hit rate appends");
    assert_eq!(cache.cache_history[1].0, 57);
}

#[test]
fn cumulative_total_survives_process_reload_and_renders_in_ledger() {
    // End-to-end pipeline evidence: transcript usage events → SessionState
    // accumulator → disk cache → FRESH process → ledger CACHE row.
    use cc_pulseline::config::GlyphMode;
    use cc_pulseline::render::color::resolve_palette;
    use cc_pulseline::render::pane::LayoutStyle;

    let workspace = TempDir::new().expect("temp workspace");
    let transcript = workspace.path().join("reload.jsonl");
    let session_id = "cache-trend-reload-ledger-2026";

    // Two usage events with distinct message ids: 1000 + 500 = 1.5k.
    append_line(&transcript, &usage_line("msg_1", 1000, "u1", 0));
    append_line(&transcript, &usage_line("msg_2", 500, "u2", 5));

    let payload_json = json!({
        "session_id": session_id,
        "transcript_path": transcript,
        "workspace": {"current_dir": workspace.path()},
    })
    .to_string();

    let ledger_cfg = || RenderConfig {
        pane_style: LayoutStyle::Ledger,
        glyph_mode: GlyphMode::Icon,
        color_enabled: false,
        terminal_width: Some(144), // ledger sees 140 after cc_margin
        pane_cc_margin: 4,
        palette: resolve_palette("tokyo-night", Some("dark"), &Default::default()),
        show_cache_trend: true,
        transcript_poll_throttle_ms: 0,
        ..RenderConfig::default()
    };

    // Process 1: reads both usage events, persists to disk cache, dies.
    {
        let mut runner1 = PulseLineRunner::default();
        let lines = runner1
            .run_from_str(&payload_json, ledger_cfg())
            .expect("first process renders");
        let blob = lines.join("\n");
        assert!(blob.contains("CACHE"), "CACHE row in process 1: {blob}");
        assert!(
            blob.contains("1.5k"),
            "cumulative 1.5k in process 1: {blob}"
        );
    } // runner1 dropped — simulates process exit

    // FRESH process, same payload, no new transcript bytes: the cumulative
    // must come back from the disk cache (offset persisted → no re-read,
    // no double-count).
    let mut runner2 = PulseLineRunner::default();
    let lines = runner2
        .run_from_str(&payload_json, ledger_cfg())
        .expect("fresh process renders");
    let blob = lines.join("\n");
    assert!(blob.contains("CACHE"), "CACHE row after reload: {blob}");
    assert!(
        blob.contains("1.5k"),
        "cumulative total must survive process reload (not double-count): {blob}"
    );

    // compact_boundary clears the trend window but NOT the cumulative.
    append_line(
        &transcript,
        &json!({
            "type": "system",
            "subtype": "compact_boundary",
            "timestamp": "2026-06-12T00:01:00.000Z"
        })
        .to_string(),
    );
    let lines = runner2
        .run_from_str(&payload_json, ledger_cfg())
        .expect("post-compaction render");
    let blob = lines.join("\n");
    assert!(
        blob.contains("1.5k"),
        "compaction must not clear the cumulative total: {blob}"
    );

    // A third usage event (new message id, wall-clock AFTER the boundary —
    // line timestamps are monotone in real transcripts) increments:
    // 1500 + 500 = 2.0k.
    append_line(&transcript, &usage_line("msg_3", 500, "u3", 70));
    let lines = runner2
        .run_from_str(&payload_json, ledger_cfg())
        .expect("post-append render");
    let blob = lines.join("\n");
    assert!(
        blob.contains("2.0k"),
        "new usage event must increment the cumulative: {blob}"
    );
}
