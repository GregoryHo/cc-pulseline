# Performance

## Targets

| Scenario | P50 Target | P95 Target | P99 Target |
|----------|-----------|-----------|-----------|
| Baseline (no activity) | < 5ms | < 10ms | < 15ms |
| Active session (10T+5A) | < 10ms | < 20ms | < 30ms |
| Large transcript (2500) | < 20ms | < 50ms | < 80ms |

> Actual numbers will be updated after running `cargo bench`.

## Methodology

- Tool: Criterion.rs + cargo bench
- Platform: (your machine spec here)
- Iterations: 100+

### Benchmark Scenarios

1. **Baseline render** -- Static JSON payload with no transcript activity. Measures pure render pipeline latency.
2. **Active session render** -- 10 active tools + 5 agents + todo items. Measures activity rendering cost.
3. **Large transcript render** -- 2500 events simulating a real transcript. Uses real payload fixture (`tests/fixtures/core_metrics_complete.json`).

## Results

| Scenario | P50 | P95 | P99 |
|----------|-----|-----|-----|
| Baseline (no activity) | TBD | TBD | TBD |
| Active session (10T+5A) | TBD | TBD | TBD |
| Large transcript (2500) | TBD | TBD | TBD |

## Regression Test

`cargo test adaptive_performance` asserts p95 < 50ms on CI. This existing test serves as the continuous integration performance gate.

## Run Locally

```bash
cargo bench
```
