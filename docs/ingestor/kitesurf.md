# Kitesurf Ingestor Dossier

**Lineage:** Kitesurf (cloudflare/kitesurf) — Cloudflare, experimental. Cloned to `forge/kitesurf`.
**Status:** `forged` → wraps for McHarness specified below.
**Forge target:** `mch-browser` (native engine backend), new `mch-render` (headless HTML/CSS/JS), `mch-cyber` (per-page sandbox model).

## What it is

Kitesurf is an **experimental browser engine that runs on Cloudflare Workers**. It
parses HTML, applies CSS, runs page JavaScript, and renders screenshots **without
Chrome or a browser container**. Critically for us, it **speaks the Chrome DevTools
Protocol (CDP)**, so existing CDP clients (Puppeteer, and our own `chromiumoxide`
usage in `mch-browser`) can drive it instead of a real Chromium.

This is the single most valuable target for McHarness's "Hands & Eyes": it collapses
the Chrome dependency that `mch-browser` inherits from `chromiumoxide`.

## Architecture

Build: Bazel + Rust (nightly) + wasm-bindgen + Node 22/pnpm. Local engine runs via
`wrangler dev` on `127.0.0.1:8787`; CDP browser WS endpoint at
`/devtools/browser`; HTTP discovery at `/json/version`. Requires Cloudflare
**Dynamic Workers** (paid) to run on Cloudflare — each page gets an isolated worker.

Rust workspace crates under `packages/`:

| Package | Role |
|---|---|
| `core` | The engine. `src/dom/` (`tree.rs`, `cdp.rs`, `input.rs`, `mutate.rs`, `scan.rs`, `shadow.rs`), `src/style/`, `src/layout/`, `src/paint/`, `src/js/`, `src/resources.rs`, `src/tty.rs`, `src/swap_alloc.rs`, `src/boa_code_store.rs` |
| `page-renderer`, `page-script`, `renderer-core` | Render + script host |
| `scene-host-wasm` | Renders into a scene (wasm) |
| `script-gate`, `worker-script` | **Per-page script isolation / gating** |
| `dom-shim`, `jsrpc-types` | DOM shim + JS-RPC type contract for CDP |
| `engine`, `kitesurf-cli` | Engine assembly + CLI |

JS execution is **Boa** (a pure-Rust JS engine) — see `packages/core/src/boa_code_store.rs`
and `src/js.rs`. This is why no V8/Chrome is needed.

## Crown Jewels

1. **Pure-Rust browser core** — DOM tree (`dom/tree.rs`), CSS style resolution
   (`style/`), layout (`layout/`), paint (`paint/`) implemented in Rust with no
   native browser process.
2. **Boa-backed page JS** — `boa_code_store.rs` + `js.rs` run untrusted page script
   through a Rust JS engine (no V8, no JIT escape surface).
3. **CDP server** — `dom/cdp.rs` + `jsrpc-types` implement the DevTools protocol
   surface (screenshots, DOM, input) so any CDP client attaches unchanged.
4. **Per-page sandboxing** — `script-gate` / `worker-script` isolate each page's
   script in its own worker; this is the "network egress denied, one-page-one-sandbox"
   pattern.
5. **Foreign-function layering** — `dom-shim` + `jsrpc-types` give a typed contract
   between the engine and the JS bridge.
6. **wasm scene host** — `scene-host-wasm` renders to a scene without a display server.

## Rust Re-implementation Targets

- **`mch-browser` (native engine backend):** add an `EngineBackend` trait with two
  impls — `ChromiumOxideBackend` (today) and a new `KitesurfBackend` that connects
  to a local Kitesurf CDP endpoint (`ws://127.0.0.1:8787/devtools/browser`) via the
  existing `chromiumoxide` CDP client. Same `Observation`/`Action` surface, no Chrome.
- **New crate `mch-render`:** embed the Kitesurf `core` crate (or a trimmed fork of
  `dom`+`style`+`layout`+`paint`+`js`) to parse→apply CSS→run Boa JS→produce a
  screenshot *in-process*, for offline page understanding with zero browser install.
- **`mch-cyber` sandbox:** adopt the `script-gate`/`worker-script` model — one
  isolated script context per target page — as the concrete implementation behind
  the existing `Sandbox` trait's per-target isolation.

## Take / Leave

- **Take:** the CDP-server-over-a-Rust-engine pattern; Boa for untrusted JS; the
  per-page worker isolation; the DOM/style/layout/paint crate split as a clean
  module boundary.
- **Leave:** the Bazel build and Cloudflare Dynamic-Workers coupling — we build the
  core with plain Cargo and run the engine as a local sidecar, not on Cloudflare.
