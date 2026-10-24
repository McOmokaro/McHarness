<div align="center">

# McHarness

**The father of harnesses.**

Built the Wozniak way: build the thing you want to use, from parts you understand, and nothing you don't. McHarness is the terminal agent I wanted — so it gets built in the open.

*One whole. Light speed. Featherweight.*

</div>

---

McHarness is a native multi-agent terminal harness forged by reverse-engineering the strongest open-source agents alive — OpenCode, Hermes, Goose, Pi, Paperclip, Deerflow, Mercury, OpenClaw, N8N, and more — and weaving their crown jewels into **one organism**. Tools, memory, skills, and inference compiled as a single system: intent in, action out.

## The Tech Stack

| Layer | What it does |
|---|---|
| **Rust** | The core: agent loop, memory, Key Farm, orchestration |
| **Python** | Research tooling, Fabric ingestion, bridge to the core |
| **C++** | The speed organs: token streaming, memory search, embedding math |
| **OpenCode** | The working agent front end during the build-out |
| **REA** | Reverse Engineer Anything — the analysis beam that feeds the Ingestor |

## Roadmap

### 🟢 DONE

- Portable Rust / Python / C++ workspace layout (`core/`, `crates/`, `cpp/`, `python/`)
- Skeleton crate set: **mch-core, mch-gate, mch-ledger, mch-scope, mch-audit, mch-compaction, mch-clock, mch-scheduler**
- **OpenCode** integrated as the live coding agent
- **REA MCP server** wired into the stack — full 207-tool inventory verified live
- **Omarchy** dual-boot environment
- **HackaTime** tracking active
- Forge Ledger public: 16 lineages mapped in [docs/forge-ledger.md](docs/forge-ledger.md) + per-agent ingestor notes in [docs/ingestor/](docs/ingestor/)
- Repository hygiene pass: single README, strict `.gitignore`, clean index

### 🟠 IN PROGRESS

- Core Rust organs — agent loop, memory, Key Farm, model layer (`core/src/`)
- C++ speed organs — token streaming, memory search, embeddings (`cpp/src/`)
- Python bridge to the core (`python/bridge.py`)

### 🔴 PLANNED

- **Key Farm** as one key pool: every provider, live rate telemetry, reroute-before-failure
- Domain **leaderboards** wired to live model rankings, fallback routing
- **Fusion**: skill corpus v1 (Fabric ingestion — lifeos, personas, decision frameworks)
- **Fission**: plugin corpus v1, one-command updates
- **Ingestor** repo-mining phase: clone → analyze → propose native re-implementations
- **TUI**: Key Farm telemetry, model-routing viz, memory/skill activity feed
- Automated bug-bounty recon with scope safety asserted in code
- Autonomous web builder
- **McOmokaro synthesis**: reverting research to native Rust, one organ at a time
- Web front door on free hosting — iPhone, Android, any browser
- One-line curl installer

## Forge Ledger

Who got mined and what was taken: [docs/forge-ledger.md](docs/forge-ledger.md).
Full architecture map: [docs/architecture.md](docs/architecture.md).

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=McOmokaro/McHarness&type=Date)](https://star-history.com/#McOmokaro/McHarness&Date)

## License

MIT — see [LICENSE](LICENSE).
