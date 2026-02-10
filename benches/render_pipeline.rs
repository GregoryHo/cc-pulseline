use criterion::{criterion_group, criterion_main, Criterion};

use cc_pulseline::config::RenderConfig;
use cc_pulseline::run_from_str;

fn baseline_payload() -> String {
    serde_json::json!({
        "session_id": "bench-baseline",
        "model": { "id": "claude-opus-4-6", "display_name": "Opus 4.6" },
        "output_style": { "name": "normal" },
        "version": "2.1.37",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 43,
            "current_usage": {
                "input_tokens": 10000,
                "output_tokens": 20000,
                "cache_creation_input_tokens": 30000,
                "cache_read_input_tokens": 40000
            }
        },
        "cost": {
            "total_cost_usd": 3.50,
            "total_duration_ms": 3600000
        }
    })
    .to_string()
}

fn active_session_payload() -> String {
    let mut tools = Vec::new();
    for i in 0..10 {
        tools.push(serde_json::json!({
            "type": "tool_use",
            "name": format!("Tool{i}"),
            "tool_use_id": format!("tool-{i}"),
            "input": { "file_path": format!("/src/file{i}.rs") }
        }));
    }

    let mut agents = Vec::new();
    for i in 0..5 {
        agents.push(serde_json::json!({
            "type": "progress",
            "data": {
                "type": "agent_progress",
                "agentId": format!("agent-{i}"),
                "status": "running",
                "prompt": format!("Investigate issue {i}"),
                "agentType": "Explore"
            }
        }));
    }

    // Build a transcript JSONL with tools and agents
    let transcript_dir = std::env::temp_dir().join("bench-active-transcript");
    std::fs::create_dir_all(&transcript_dir).ok();
    let transcript_path = transcript_dir.join("transcript.jsonl");

    let mut lines = Vec::new();
    // Tool use events
    for (i, tool) in tools.iter().enumerate() {
        lines.push(
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "content": [tool]
                }
            })
            .to_string(),
        );
        // Add some completed tools
        if i < 5 {
            lines.push(
                serde_json::json!({
                    "type": "user",
                    "message": {
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": format!("tool-{i}"),
                            "content": "ok"
                        }]
                    }
                })
                .to_string(),
            );
        }
    }
    // Agent progress events
    for agent in &agents {
        lines.push(agent.to_string());
    }

    std::fs::write(&transcript_path, lines.join("\n") + "\n").ok();

    serde_json::json!({
        "session_id": "bench-active",
        "transcript_path": transcript_path.to_str().unwrap(),
        "model": { "id": "claude-opus-4-6", "display_name": "Opus 4.6" },
        "output_style": { "name": "normal" },
        "version": "2.1.37",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 65,
            "current_usage": {
                "input_tokens": 50000,
                "output_tokens": 30000,
                "cache_creation_input_tokens": 20000,
                "cache_read_input_tokens": 80000
            }
        },
        "cost": {
            "total_cost_usd": 8.75,
            "total_duration_ms": 1800000
        }
    })
    .to_string()
}

fn large_transcript_payload() -> String {
    let transcript_dir = std::env::temp_dir().join("bench-large-transcript");
    std::fs::create_dir_all(&transcript_dir).ok();
    let transcript_path = transcript_dir.join("transcript.jsonl");

    let mut lines = Vec::new();
    let tool_names = ["Read", "Write", "Edit", "Bash", "Glob", "Grep", "Task"];
    for i in 0..2500 {
        let name = tool_names[i % tool_names.len()];
        let id = format!("tool-{i}");
        // Tool use
        lines.push(
            serde_json::json!({
                "type": "assistant",
                "message": {
                    "content": [{
                        "type": "tool_use",
                        "id": &id,
                        "name": name,
                        "input": { "file_path": format!("/src/file{i}.rs") }
                    }]
                }
            })
            .to_string(),
        );
        // Tool result
        lines.push(
            serde_json::json!({
                "type": "user",
                "message": {
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": &id,
                        "content": "ok"
                    }]
                }
            })
            .to_string(),
        );
    }

    std::fs::write(&transcript_path, lines.join("\n") + "\n").ok();

    serde_json::json!({
        "session_id": "bench-large",
        "transcript_path": transcript_path.to_str().unwrap(),
        "model": { "id": "claude-opus-4-6", "display_name": "Opus 4.6" },
        "output_style": { "name": "normal" },
        "version": "2.1.37",
        "context_window": {
            "context_window_size": 200000,
            "used_percentage": 85,
            "current_usage": {
                "input_tokens": 100000,
                "output_tokens": 50000,
                "cache_creation_input_tokens": 40000,
                "cache_read_input_tokens": 150000
            }
        },
        "cost": {
            "total_cost_usd": 25.00,
            "total_duration_ms": 7200000
        }
    })
    .to_string()
}

fn bench_baseline_render(c: &mut Criterion) {
    let payload = baseline_payload();
    let config = RenderConfig::default();

    c.bench_function("baseline_render", |b| {
        b.iter(|| {
            run_from_str(&payload, config.clone()).unwrap();
        })
    });
}

fn bench_active_session_render(c: &mut Criterion) {
    let payload = active_session_payload();
    let config = RenderConfig::default();

    c.bench_function("active_session_render", |b| {
        b.iter(|| {
            run_from_str(&payload, config.clone()).unwrap();
        })
    });
}

fn bench_large_transcript_render(c: &mut Criterion) {
    let payload = large_transcript_payload();
    let config = RenderConfig::default();

    c.bench_function("large_transcript_render", |b| {
        b.iter(|| {
            run_from_str(&payload, config.clone()).unwrap();
        })
    });
}

criterion_group!(
    benches,
    bench_baseline_render,
    bench_active_session_render,
    bench_large_transcript_render,
);
criterion_main!(benches);
