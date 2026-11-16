# pxpipe Ingestor Dossier

**Lineage:** pxpipe (teamchong/pxpipe) — MIT Licensed, TypeScript/Node.js. Cloned to `forge/pxpipe`.
**Status:** `forged` → wraps for McHarness specified below.
**Forge target:** `mch-core` (local proxy for context compression), `mch-tui` (visualizer for token savings), `mch-brain` (context compression strategy).

## What it is

**pxpipe** is a **local proxy that compresses LLM request context by rendering bulky text (system prompt, tool docs, history) into dense PNG images**. The model reads the images via its vision encoder instead of reading raw text tokens.

- **Token savings:** ~59–70% lower end-to-end bill on `claude-fable-5` (10/10 SWE-bench Lite, 14/19 SWE-bench Pro).
- **Gemini 4 (`gemini-4-argon`)** reaches **15/15** on pure-image dense hex; later Claude releases regressed sharply (0/15–3/15 hex).
- **Context window expansion:** ~4.8× (Fable 5) to 5.3× (Gemini 3.6 Flash) effective text capacity via dense 5×8 Spleen font rendering.
- **Zero base-URL overrides** — `ANTHROPIC_BASE_URL` / `GEMINI_API_BASE` stay unset; `agy` OAuth, Claude Code `/remote-control`, claude.ai connectors work unchanged.

## Architecture

**Core Components:**

| Component | Role |
|---|---|
| `pxpipe-proxy` | Local HTTP/HTTPS proxy on `127.0.0.1:47821`; intercepts Anthropic/Google/OpenAI-compatible `/messages`/`generateContent` calls |
| `pxpipe-proxy export` | Offline export: renders text/files/diffs to PNG pages without proxy |
| `pxpipe warp -- agy\|omp\|opencode\|claude\|codex` | Zero-config warp for Antigravity/omp/OpenCode/Claude Code/Codex |
| `pxpipe-proxy export` | Offline export: renders text/files/diffs to PNG pages without proxy |
| Dashboard | `http://127.0.0.1:47821/` — tokens saved, side-by-side text↔image, kill switch, live model chips |

**Request Transformation Pipeline:**
```
Client Request → pxpipe-proxy → [Detect bulky fields] → Render to PNG (Spleen 5×8 font) → Replace text with image → Forward to provider
```

**Key Design Decisions:**
1. **Compresses REQUEST only** — model output streams as text unchanged.
2. **Recent turns stay text** — only system prompt, tool docs, and older bulk history are imaged.
3. **Zero base-URL overrides** — `ANTHROPIC_BASE_URL` / `GEMINI_API_BASE` stay unset; `agy` OAuth, Claude Code `/remote-control`, claude.ai connectors work unchanged.
3. **Zero base-URL overrides** — `ANTHROPIC_BASE_URL` / `GEMINI_API_BASE` stay unset; `agy` OAuth, Claude Code `/remote-control`, claude.ai connectors work unchanged.
4. **`pxpipe warp`** — zero-config wrapper: `pxpipe warp -- agy|omp|opencode|claude|codex` sets up proxy + model routing automatically.
5. **Route rules** — custom `--route` rules for custom base URLs (e.g., OpenCode on local port).
6. **`pxpipe-proxy export`** — offline export: renders text/files/diffs to PNG pages without proxy.

**Model Tiers (from README):**
- **Tier 1 — Full-fidelity readers** — `gemini-4-argon` (15/15 hex, 100/100 arithmetic), `gemini-3.6/3.7/3.8-flash` (14/15 hex), `claude-fable-5` (13/15 hex) — **On by default**
- **Tier 2 — Gist & agentic history only** — `claude-opus-5-5`, `claude-fable-5-1/5-2`, `claude-opus-5`, `grok-4.5/4.6` — **Gist/agentic history only, not byte-exact on dense logs/JSON**
- **Unsupported — Do not image** — `claude-haiku-5-5`, `claude-sonnet-5-5`, `claude-opus-4-8`, `gpt-6.1-sol`, `grok-4.7`, `gpt-6.1-sol`, `kimi-k3`, `glm-5.3-flash`, `deepseek-v4.1-flash-1017` — **Do not image** (weak dense pixel reading)

**Routing:**
- `pxpipe warp -- agy` → Antigravity CLI (Gemini 4 / 3.8 Flash via CloudCode PA)
- `pxpipe warp -- omp` → omp (Anthropic Messages, OpenAI Responses, or Google CloudCode PA)
- `pxpipe warp -- opencode` → OpenCode (or: claude, codex, cursor-agent, or any shell alias)
- Custom `--route` rules for custom base URLs

**Model Tiers & Rendering Profiles:**
| Tier | Models | Dense Hex (N=15) | Arithmetic (N=100) | Gist (N=98) | Profile |
|---|---|---|---|---|---|
| Tier 1 | `gemini-4-argon`, `gemini-3.6/3.7/3.8-flash`, `claude-fable-5` | 15/15, 14/15, 13/15 | 100/100 | 98/98 | Shipped |
| Tier 1 | `claude-fable-5-1/5-2` (spaced) | 14/15 | 100/100 | 98/98 | Spaced profile |
| Tier 2 | `claude-opus-5-5`, `claude-fable-5-1/5-2`, `claude-opus-5`, `grok-4.5/4.6` | 0-14/15 | 100/100 | 83-97/98 | Opt-in |
| Unsupported | `claude-haiku-5-5`, `claude-sonnet-5-5`, `claude-opus-4-8`, `gpt-6.1-sol`, `grok-4.7`, `kimi-k3`, `glm-5.3-flash`, `deepseek-v4.1-flash-1017` | 0-2/15 | 36-100/100 | 14-98/98 | **Do not image** |

**Offline Export:** `pxpipe-proxy export src/` / `cat prompt.txt | pxpipe-proxy export --stdin` / `pxpipe-proxy export --git` — writes `pxpipe-export-XXXXXX/` with `page-*.png`, `factsheet.txt`, `manifest.json`, `prompt.txt`.

## Crown Jewels

1. **Token-Compression-by-Image** — Rewrites bulky request context (system prompt, tool docs, history) into dense PNG pages read by vision encoder. Fixed pixel cost regardless of text length. ~3.1 chars/image-token vs ~1 char/text-token.

2. **Zero Base-URL Overrides** — Keeps `ANTHROPIC_BASE_URL` / `GEMINI_API_BASE` unset. Works with `agy` OAuth, Claude Code `/remote-control`, claude.ai connectors, first-party gates. No base-URL override needed.

3. **Zero-Config Warp** — `pxpipe warp -- agy|omp|opencode|claude|codex` auto-configures proxy + model routing. Zero base-URL overrides.

4. **Model-Aware Rendering Profiles** — Per-model rendering profiles (Spleen 5×8 312-col for Claude/Gemini; JetBrains Mono 14px/84-col for GPT/Grok). Per-model rendering profiles with quality receipts.

4. **Model-Aware Rendering Profiles** — Per-model rendering profiles (Spleen 5×8 312-col for Claude/Gemini; JetBrains Mono 14px/84-col for GPT/Grok). Per-model quality receipts in `eval/`.

4. **Per-Model Quality Receipts** — Every model has a quality receipt (arithmetic, gist, state, dense hex, state, never-stated) in `eval/`. Tier 1 models ship with quality receipts; Tier 2 opt-in; Unsupported blocked.

5. **Zero-Config Warp** — `pxpipe warp -- agy|omp|opencode|claude|codex` auto-configures proxy + model routing. Zero base-URL overrides.

5. **Offline Export** — `pxpipe-proxy export` renders text/files/diffs to PNG pages without proxy running.

6. **Per-Model Quality Receipts** — Every model has a quality receipt (arithmetic, gist, state, dense hex, state, never-stated) in `eval/`. Tier 1 models ship with quality receipts; Tier 2 opt-in; Unsupported blocked.

5. **Model-Aware Rendering Profiles** — Per-model rendering profiles (Spleen 5×8 312-col for Claude/Gemini; JetBrains Mono 14px/84-col for GPT/Grok). Per-model quality receipts in `eval/`.

6. **Model-Aware Routing** — `PXPIPE_MODELS` env var controls which models get imaged. Default: `claude-fable-5,claude-opus-5-5,gemini`. Opt-in: Opus 5, Sonnet 5.5, Grok 4.5/4.6, GPT-5.6, Grok 4.5/4.6. `PXPIPE_MODELS=off` disables.

7. **Profitability Gate** — Only images where math wins (token-dense content). Calibrated on N=391 production rows.

7. **Workload-Dependent Profitability** — Wins on token-dense content (~1 char/token), loses money on sparse prose (~3.5 chars/token). Profitability gate calibrated on N=391 production rows.

8. **Client-Dependent Savings** — Savings track uncached bulk the client still re-sends as text. Claude Code re-sends system+tools+history on `/anthropic/messages` typically 60–70%.

## Rust Re-implementation Targets

| Crown Jewel | Target Crate | Notes |
|---|---|---|
| **Local Proxy Server** | `mch-core` | `mch-core` gets a `PxpipeProxy` module: HTTP/HTTPS proxy that intercepts Anthropic/Google/OpenAI-compatible `/messages`/`generateContent` calls |
| **Image Renderer** | `mch-render` / `mch-core` | Dense text → PNG via Spleen 5×8 / JetBrains Mono fonts; configurable DPI, font, page size |
| **Model-Aware Rendering Profiles** | `mch-brain` / `mch-core` | Per-model rendering profiles (Spleen 5×8 312-col for Claude/Gemini; JetBrains Mono 14px/84-col for GPT/Grok) with quality receipts |
| **Zero-Config Warp** | `mch-core` | `PxpipeWarp` — zero-config wrapper that spawns proxy + routes model traffic |
| **Offline Export** | `mch-core` / `mch-tui` | `pxpipe export` — render text/files/diffs to PNG pages without proxy |
| **Model-Aware Routing** | `mch-core` | `PXPIPE_MODELS` env var → model-to-profile routing; Tier 1/2/Unsupported gating |
| **Profitability Gate** | `mch-core` | Only compress where math wins (token-dense content); calibrated on N=391 production rows |
| **Model-Aware Routing** | `mch-core` | `PXPIPE_MODELS` env var → model-to-profile routing; Tier 1/2/Unsupported gating |
| **Profitability Gate** | `mch-core` | Only compress where math wins (token-dense content); calibrated on N=391 production rows |
| **Client-Dependent Savings** | `mch-core` / `mch-tui` | Savings track uncached bulk the client still re-sends as text; dashboard shows tokens saved |
| **Dashboard** | `mch-tui` | Live tokens saved, side-by-side text↔image, kill switch, live model chips |

**Files to Reference in Forge:**
- `src/proxy.ts` — proxy server, request interception
- `src/renderer.ts` — text→PNG rendering (Spleen/JetBrains Mono)
- `src/warp.ts` — `pxpipe warp` command
- `src/export.ts` — offline export
- `src/profiles/` — per-model rendering profiles
- `eval/` — quality receipts per model
- `src/proxy.ts` — proxy server, request interception
- `src/renderer.ts` — text→PNG rendering (Spleen/JetBrains Mono)
- `src/warp.ts` — `pxpipe warp` command
- `src/export.ts` — offline export
- `src/profiles/` — per-model rendering profiles
- `eval/` — quality receipts per model

## Take / Leave

- **Take:** The proxy-based compression architecture, per-model rendering profiles with quality receipts, zero-config warp, profitability gate, zero base-URL overrides, offline export.
- **Leave:** The TypeScript/Node.js runtime (port to Rust), the Apify/Apify-actor dependency (we'll use native Rust), the Skool community marketing, the specific font choices (we'll pick our own fonts), the dashboard UI (we have our own TUI).

---

### Files to Reference in Forge

- `src/proxy.ts` — proxy server, request interception
- `src/renderer.ts` — text→PNG rendering (Spleen/JetBrains Mono)
- `src/warp.ts` — `pxpipe warp` command
- `src/export.ts` — offline export
- `src/profiles/` — per-model rendering profiles
- `eval/` — quality receipts per model