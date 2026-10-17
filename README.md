<div align="center">

# McHarness

**The father of harnesses.**

*One whole. Light speed. Featherweight.*

[![Star History Chart](https://api.star-history.com/svg?repos=McOmokaro/McHarness&type=Date)](https://star-history.com/#McOmokaro/McHarness&Date)

</div>

---

McHarness is a native multi-agent terminal harness forged by reverse-engineering the best open-source agents alive — OpenCode, Hermes, Goose, Pi, Paperclip, Deerflow, Mercury, OpenClaw, N8N, and more — and weaving their crown jewels into **one organism**.

It is not a tool-calling agent. It is **Whole**: tools, memory, skills, and inference compiled into a single system — you press the pedal, it accelerates. No visible seams. Light-speed tool acts. Low latency.

## The Differentiators

| Forge | What it does |
|---|---|
| **The Whole** | Tools, memory, skills, inference — one compiled organism. Tool calls happen at light speed; it doesn't feel like tool calling at all. |
| **The Key Farm** | All API keys act as one pool. Name the model — the farm finds every key that carries it, lines up fallbacks, watches rate limits live, and reroutes before failure. Leaderboards pick the next best proven model when one dies. |
| **Fusion** | All skills ever released, reverse-engineered and bundled as one core. Type your problem — every skill for that domain is already armed. One command updates. |
| **Fission** | The same pattern for plugins — all plugins as one, domain-addressed, one-command updates. |
| **The Ingestor** | Feed it any open-source agent; it reverse-engineers it and integrates it seamlessly into the ecosystem. |
| **Omarchy Featherweight** | Tiny download. Every capability is a placeholder that self-installs only when first needed. |
| **Everywhere** | Native terminal core; hosted web front door (free hosting) after — iPhone, Android, any browser. Billions of devices. |

## Forge Ledger

See [docs/forge-ledger.md](docs/forge-ledger.md) — the public map of which capability was mined from which lineage.

## Architecture

See [docs/architecture.md](docs/architecture.md).

- **Rust (70%)** — agent loop, memory, model layer, Key Farm, orchestration
- **C++ (20%)** — the speed organs: token streaming, memory search, embedding math
- **Python (10%)** — research tooling, Fabric ingestion, bridge to the core

## Principles

1. **Whole by design** — no visible seams between tools, memory, and inference.
2. **Model-agnostic from birth** — any provider, any key, any model.
3. **Featherweight** — nothing is installed until it is needed.
4. **Scope safety** — security specialists act only on explicitly authorized targets.
5. **Open source** — built in the open, attributed in the Ledger.

## Status

`v0.0.1 — Skeleton. The organs are next.`

## License

MIT — see [LICENSE](LICENSE).
