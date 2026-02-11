# Performance

## Render Budget

This binary is called repeatedly by a statusline hook — it must be fast:
- **Target**: p95 < 50ms end-to-end
- **No synchronous network I/O** — ever
- **No full-file reads** when incremental parsing works
- **No blocking waits** — fail fast with cached/default data

## Transcript Parsing

The transcript file can grow to thousands of lines. ALWAYS use incremental parsing:
- Seek to last read offset (`SessionState.transcript_offset`)
- Parse only new lines since last invocation
- Apply event windowing (`transcript_window_events`) to cap memory
- Detect file truncation and reset offset to 0

## Caching Strategy

- **Env/Git snapshots**: 10s TTL (`CACHE_TTL_MS`) — avoid re-scanning on every call
- **Session state**: persisted to `{temp_dir}/cc-pulseline-{hash}.json`
- **Atomic writes**: write to `.tmp` file, then `rename()` — prevents partial reads
- **Silent failure**: all cache load/save errors are intentionally ignored — never crash on cache

## Allocation Discipline

- Use `Vec::with_capacity()` when the size is known or estimable
- FIFO caps on completed agents (max 10) and completed tools (`max_completed_tools`)
- Avoid `.clone()` in hot paths — prefer references or `Cow<str>`
- Reuse `SessionState` across invocations (persist + reload)

## Regression Testing

Before merging performance-sensitive changes:
- Run `cargo bench` before and after
- Check the 2500-event budget test in `tests/adaptive_performance.rs`
- Profile with `cargo test --release` if render time increases
