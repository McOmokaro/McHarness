# Modern Web Guidance Ingestor Dossier

**Lineage:** Modern Web Guidance (GoogleChrome/modern-web-guidance) — MIT Licensed, maintained by Google Chrome, Microsoft Edge, Mozilla Firefox, and MDN teams. Cloned to `forge/modern-web-guidance`.
**Status:** `forged` → wraps for McHarness specified below.
**Forge target:** `mch-browser` (modern web guidance skill), `mch-tui` (CSS/HTML reference), `mch-brain` (skill system).

## What it is

Modern Web Guidance is a **curated skill library** that embeds web platform expertise, best practices, and browser compatibility data directly into coding agents. It steers agents away from legacy patterns (2015-era code) toward modern, native web platform solutions.

It is maintained by Google Chrome, Microsoft Edge, Mozilla Firefox, and MDN teams alongside the web dev community. The source of truth is the web platform itself (web-platform-dx.github.io).

## Architecture

**CLI Tool** (`npx modern-web-guidance@latest`):
- `install` — installs the skill into an agent's context
- `search "query"` — finds relevant guides by natural language query
- `retrieve <guide-id>` — retrieves a specific guide by ID
- `legacy-scan .` — scans a codebase for legacy patterns to modernize

**Skill Store** (under `skills/modern-web-guidance/`):
- 250 modern web features across 5 core disciplines:
  - **CSS & Layout (140 features)**: container queries, subgrid, cascade layers, cascade layers, logical properties, @scope, @container, color-mix(), okLCh, aspect-ratio, @property, @scope, @starting-style, @property, etc.
  - **HTML & DOM (36 features)**: <dialog>, <slot>, <template for>, <search>, Popover API, declarative shadow DOM, inert, form-associated custom elements, form-associated custom elements, etc.
  - **JavaScript & APIs (74 features)**: Temporal, Scheduler API, Navigation API, Speculation Rules, WebGPU, WebGPU, WebGPU, Web Authentication, WebAuthn, Credential Management, Payment Request, WebAuthn, WebXR, WebHID, Web Serial, Web Bluetooth, Web NFC, Web USB, Web Bluetooth, WebHID, Web Serial, Web NFC, etc.
  - **Accessibility (12 features)**: :focus-visible, :focus-within, inert, :user-valid/:user-invalid, :autofill, accessible-error-announcement, etc.
  - **Built-in AI (4 features)**: LanguageModel, Summarizer, Translator, LanguageDetector, Writer, Rewriter

**Architecture:**
- **Skills** (under `skills/modern-web-guidance/guides/`) — each guide is a markdown file with a specific structure: problem, solution, code example, browser support, polyfill/fallback strategy.
- **Skills Index** — `skills.json` maps feature IDs to guide paths.
- **Eval Dashboard** — `eval/` contains benchmarks for agent performance on modern web tasks.

## Crown Jewels

1. **250+ Modern Web Features** — Comprehensive, curated coverage of the modern web platform (CSS, HTML, JS, APIs, Accessibility, Built-in AI). Each feature has a guide with: browser support table, code examples, polyfill strategy, accessibility notes, and migration path from legacy patterns.

2. **Legacy Pattern Detection** — `legacy-scan` command scans a codebase for legacy patterns (e.g., jQuery, old CSS hacks, deprecated APIs) and suggests modern replacements with exact file/line references.

3. **Token-Efficient Guides** — Guides are pruned via evals to be token-efficient. They focus on what models *don't* already know (e.g., they skip basic flexbox but deep-dive into container queries).

3. **Browser Compatibility Data** — Each guide includes a machine-readable browser support matrix (via `web-platform-dx` data source), enabling agents to check compatibility at call time.

4. **Agent Skill Protocol** — The `modern-web-guidance` npm package installs a skill that exposes `search`, `retrieve`, and `legacy-scan` as MCP-like tools. The `llms.txt` file provides a plain-text index for LLM context.

4. **Eval Dashboard** — Automated evals measure how well agents use modern web features vs. legacy patterns.

## Rust Re-implementation Targets

| Crown Jewel | Target Crate | Notes |
|---|---|---|
| **Feature Database** (250+ features with metadata) | `mch-brain` | Store as searchable skill corpus; `mch-brain` skill system can index and retrieve |
| **Legacy Pattern Scanner** | `mch-loop` / `mch-core` | `legacy-scan` equivalent for Rust codebases (detect legacy Rust patterns) |
| **Feature Database Query API** | `mch-core` | `search(query)`, `retrieve(id)`, `legacy_scan(path)` functions |
| **Skill Manifest** | `mch-tui` | Display available modern web features in TUI |
| **MCP Skill Interface** | `mch-mcp` | Expose `search`, `retrieve`, `legacy_scan` as MCP tools |

**Files to reference:**
- `skills/modern-web-guidance/guides/` — 250+ markdown guides
- `skills/modern-web-guidance/skills.json` — feature index
- `src/index.ts` — CLI entry point
- `src/commands/` — search, retrieve, legacy-scan commands
- `skills/modern-web-guidance/skills.json` — feature index with metadata

## Take / Leave

- **Take:** The feature taxonomy (5 disciplines × 250 features), the guide structure (problem → solution → code → browser support → fallback), the legacy-scan concept, the skill protocol (search/retrieve/legacy-scan).
- **Leave:** The npm/Node.js runtime dependency (we'll port to Rust), the npm package distribution, the specific guide prose (we'll rewrite in our voice), the npm package manager integration.

---

### Files to Reference in Forge

- `skills/modern-web-guidance/guides/` — 250+ markdown guides
- `skills/modern-web-guidance/skills.json` — feature index
- `src/commands/search.ts`, `retrieve.ts`, `legacy-scan.ts`
- `package.json` — dependencies, scripts
- `.github/workflows/` — CI for eval dashboard