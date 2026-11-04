# The Forge Roadmap

The chronological blueprint of agents reverse-engineered into the McHarness ecosystem.

## Phase 1: Core Harnesses (The Body)
- [ ] **OpenCode** — Extract streaming tool-fork loop and terminal UX.
- [ ] **Hermes Agent** — Extract skill-sealing and the self-improvement memory loop.
- [x] **Goose** — `forged` (mch-mcp): Extract MCP tooling integration.
- [ ] **Paperclip** — Extract multi-agent orchestration, budgets, and approvals.
- [ ] **Stageshow (benjitaylor)** — Faux UI stills, animations, and bento grids for Anti-Slop frontend generation.

## Phase 2: The Cybersecurity Department (The Weapons) — `in progress`
- [ ] **REA (morluto/rea)** — `forged` (mch-rea): Reverse-engineer the behavioral analysis and executable inspection loop.
- [ ] **Cloudflare security-audit-skill** — `in progress`: Extract the multi-stage audit pipeline and machine-readable vulnerability schema.
- [ ] **Hexstrike** — `in progress`: Forge the offensive tooling orchestration (scoped).
- [ ] **Bug-Bounty Agents** — `in progress`: Integrate fail-closed scope authority and target discovery.
- [ ] **OpenWorker** — `in progress`: Forge the self-waking task execution and standing approvals.

## Phase 3: The Data & UI Layer (The Senses)
- [ ] **Graphify** — Extract local source-code-to-knowledge-map conversion.
- [ ] **Hyperframes** — Forge HTML-to-video rendering for agent reporting.
- [ ] **Fabric (Daniel Miessler)** — Fold LifeOS, personas, and decision frameworks into Fusion.

## Phase 4: The TUI Engine — Telegram Desktop
- [ ] **Telegram Desktop (tdesktop)** — Reverse-engineer the native desktop client for TUI construction:
  - **Speed**: Zero-latency rendering via custom C++/Qt scene graph; study the message-list virtualization and diff-based repaint.
  - **Lightweight**: <100 MB install, minimal deps; extract the resource loading strategy and lazy module initialization.
  - **Animation**: Physics-based spring curves (mtproto's animation driver); port the easing library to the C++ speed layer.
  - **Small storage**: SQLite + custom binary serialization (tdb); study the schema and compression for local state.
  - **Cross-platform**: Windows/macOS/Linux single codebase; extract the platform abstraction layer.

## Phase 5: The Download Subsystem — Aria2c
- [ ] **Aria2c** — Integrate the multi-protocol download engine as a native capability:
  - **Metalink + BitTorrent + HTTP/HTTPS/FTP** unified in one binary.
  - **Segmented downloading** (16 connections by default) for GitHub releases — repos clone in seconds, not minutes.
  - **RPC/JSON-RPC + daemon mode** — embed as a background service the Key Farm and Ingestor can call.
  - **Checkpoint/resume** — `.aria2` control files enable crash-safe resume for any file.
  - **GitHub optimization** — `--header` auth, `--max-connection-per-server`, and `--min-split-size` tuned for large repo archives.

## Phase 6: Resilient Download Semantics — yt-dlp (YTDLnis)
- [ ] **yt-dlp** — Reverse-engineer the world's most resilient video downloader and generalize:
  - **Fragmented resume** — HLS/DASH segments tracked individually; network failure resumes *exact* byte range, never restarts.
  - **Retries with backoff** — Exponential backoff + jitter, per-fragment, configurable via `--retries`/`--fragment-retries`.
  - **Cookie/token refresh mid-download** — Auth headers refreshed on 401/403 without losing progress.
  - **External downloader hook** — `--external-downloader aria2c` already supported; we make this the *default path* for *all* file downloads in McHarness.
  - **Generalization** — Extract the **DownloadSession** abstraction (manifest → fragments → checkpoint → resume) and apply to:
    - Model weight downloads (Hugging Face, Ollama layers)
    - Dataset pulls (OSM extracts, Common Crawl shards)
    - Release artifacts (GitHub, GitLab, S3)
    - Any large asset the Ingestor or agents need
  - **Content-creator impact** — Video generation pipelines (Sora, Veo, Kling, Runway) download multi-GB assets; resume-on-failure cuts re-download from 100% → 0%.

## Phase 7: The Wildcards (To Be Analyzed)
- [ ] **IBM Bob** — Analyze capabilities.
- [ ] **Blackbox AI** — Analyze capabilities.
- [ ] **Kali Linux AI** — Extract local offensive security OS integrations for seamless binary compatibility.

## Phase 8: DeepSeek Harness (DSH) — The Cordis Kernel
- [x] **DeepSeek Harness (DSH)** — `forged` (mch-dsh-cordis): Extract the Cordis kernel "everything is a plugin" architecture:
  - **Cordis Kernel**: Everything-is-a-plugin architecture; core is minimal, all capabilities (LLM, tools, memory, skills) are plugins.
  - **BYOK Provider Abstraction**: Bring Your Own Key — unified provider interface where users plug in their own API keys; no vendor lock-in.
  - **Super-Agent Routing**: Intelligent model routing based on task type, cost, latency, and capability tags.
  - **Plugin Manifest Schema**: Standardized plugin manifest (YAML/JSON) with capabilities, dependencies, permissions, and versioning.
  - **Hot-reloadable Plugins**: Plugins can be loaded/unloaded/reloaded at runtime without restarting the agent.