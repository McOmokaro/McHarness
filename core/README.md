# McHarness Core — Rust skeleton (v0.0.1)

The agent loop, memory, model layer, and Key Farm — compiled as one Whole.

## Modules

- `loop/` — the plan-act-observe agent cycle
- `memory/` — persistent / working / skill strata (SQLite-backed)
- `model/` — model-agnostic connection layer with fallback
- `keyfarm/` — pooled API keys, rate telemetry, leaderboard fallback

## Run

```bash
cargo run --example skeleton
```
