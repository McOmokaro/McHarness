# REA: Reverse Engineer Anything

**Source:** `forge/rea` (morluto/rea)
**Forge Target:** `mch-rea` crate
**Status:** Extracted & Forged

---

## Overview

REA is an MCP (Model Context Protocol) server that connects AI agents to professional reverse engineering tooling. It provides a unified interface for static analysis (Ghidra, IDA Pro, Hopper, Binary Ninja, Cutter, radare2/rizin, JEB, JADX), dynamic analysis (Frida, GDB, LLDB), and mobile/Android tooling (ADB, apktool, objection).

**Key insight:** REA doesn't do reverse engineering itself. It exposes RE tools as MCP *tools* so an LLM agent can drive them. The agent decides what to analyze; REA executes.

---

## Architecture

### MCP Server (`src/main.ts` → `src/server/`)

```
┌─────────────────────────────────────────────────────────────┐
│                      REA MCP Server                         │
├─────────────────────────────────────────────────────────────┤
│  Transport: stdio (local) / WebSocket (remote)             │
│  Protocol: JSON-RPC 2.0                                     │
├─────────────────────────────────────────────────────────────┤
│  Tool Registry (230+ tools)                                 │
│  ├── Binary: open/close, segments, procedures, names       │
│  ├── Static Analysis: decompile, call graph, xrefs, strings │
│  ├── Dynamic: Frida sessions, GDB/LLDB, process control    │
│  ├── Mobile: ADB screen capture, activity launch, logcat   │
│  ├── Evidence: export/import bundles (forensic snapshots)  │
│  └── Native: ELF/PE/Mach-O/DEX parsing, symbol resolution  │
└─────────────────────────────────────────────────────────────┘
```

### Core Components

| Module | Purpose |
|--------|---------|
| `src/application/reverse/ReverseEngineeringService.ts` | Orchestrates multi-tool RE workflows |
| `src/native/` | ELF, PE, Mach-O, DEX parsers (pure TS) |
| `src/composition/reverseEngineering.ts` | High-level RE recipes |
| `src/domain/binaryTargetTypes.ts` | Unified binary abstraction |
| `src/contracts/` | Zod schemas for all tool I/O |
| `src/server/` | MCP server implementation |

### Binary Target Abstraction

REA normalizes across formats via `BinaryTarget`:

```typescript
interface BinaryTarget {
  handle: string;
  format: "ELF" | "PE" | "MachO" | "DEX" | "WASM" | "ARTIFACT";
  architecture: Architecture;
  bits: 32 | 64;
  entryPoint: string;
  segments: Segment[];
  symbols: Symbol[];
}
```

All tools operate on `handle` — no format-specific logic leaks to the agent.

---

## Tool Taxonomy (230+ Tools)

### Binary Management
| Tool | Description |
|------|-------------|
| `open_binary` | Load ELF/PE/Mach-O/DEX/WASM, return handle |
| `close_binary` | Release handle |
| `list_segments` | Memory map: permissions, VA, size, file offset |
| `list_procedures` | All functions with addresses, names, sizes |
| `list_names` | All named symbols (functions, globals, sections) |

### Static Analysis
| Tool | Description |
|------|-------------|
| `decompile_function` | Pseudocode at address (Ghidra/IDA/Hopper/rizin) |
| `analyze_function` | Full auto-analysis: CFG, vars, types, calls |
| `get_call_graph` | Callers/callees up to depth N |
| `find_xrefs_to_name` | All references to a symbol |
| `search_strings` | Regex search in .rodata/.data |
| `set_address_name` | Rename symbol (persists in evidence bundle) |

### Dynamic Analysis
| Tool | Description |
|------|-------------|
| `start_frida_session` | Attach to process (local/USB/remote) |
| `load_frida_script` | Inject JS, hook functions, trace execution |
| `capture_adb_screen` | Screenshot Android device |
| `start_adb_activity` | Launch activity via am |
| `read_adb_logcat` | Filtered logcat stream |

### Evidence Bundles
| Tool | Description |
|------|-------------|
| `export_evidence_bundle` | `.rea.zip`: binary + annotations + decompilations + Frida traces |
| `import_evidence_bundle` | Restore session, continue analysis |

**Bundle format:** Zip with `manifest.json`, `binary/`, `annotations/`, `decompilations/`, `traces/`. Portable, versioned, agent-readable.

---

## Native Parsers (Pure TypeScript)

REA implements binary parsers in TS — no native deps for basic inspection:

| Format | Parser | Key Features |
|--------|--------|--------------|
| ELF | `src/native/elf/` | Sections, symbols, relocations, notes, dynamic tags |
| PE | `src/native/pe/` | Sections, imports, exports, resources, certificates |
| Mach-O | `src/native/macho/` | Load commands, segments, symbols, dyld info |
| DEX | `src/native/dex/` | Classes, methods, strings, type IDs, proto IDs |

These enable *fast triage* without spinning up Ghidra/IDA.

---

## Composition Layer: Recipes

`src/composition/reverseEngineering.ts` defines reusable workflows:

```typescript
// Example: Full function analysis recipe
async function analyzeFunctionRecipe(handle: string, address: string) {
  await analyze_function(handle, address);        // Auto-analyze
  const pseudocode = await decompile_function(handle, address);
  const callers = await get_call_graph(handle, address, 2);
  const xrefs = await find_xrefs_to_name(handle, getName(address));
  return { pseudocode, callers, xrefs };
}
```

**Patterns extracted for McHarness:**
- **Probe → Analyze → Generate → Validate** loop (mirrors DSH workflow compiler)
- **Versioned evidence bundles** as immutable artifacts
- **Tool registry** with capability discovery (`list_tools`)
- **Handle-based resource management** (open → use → close)

---

## Forge Mapping: REA → mch-rea

| REA Concept | mch-rea Implementation |
|-------------|------------------------|
| MCP stdio transport | `StdioTransport` (spawns `rea` binary) |
| MCP WebSocket transport | `WebSocketTransport` (remote REA) |
| Tool registry | `list_tools()` → `Vec<McpTool>` |
| Binary handle | Opaque `String` passed to all tools |
| Evidence bundle | `export_evidence_bundle` / `import_evidence_bundle` |
| Frida session | `start_frida_session` → session ID → `load_frida_script` |
| ADB integration | `capture_adb_screen`, `start_adb_activity`, `read_adb_logcat` |
| Native parsers | **Not ported** — use REA MCP for heavy lifting; mch-rea is a *client* |

### mch-rea Crate Structure

```
crates/mch-rea/
├── src/
│   ├── lib.rs          // Re-exports
│   ├── protocol.rs     // JSON-RPC 2.0 + MCP types (Initialize, Tool, Resource, Prompt)
│   ├── transport.rs    // StdioTransport, WebSocketTransport
│   ├── client.rs       // McpClient: request/response, notifications, typed calls
│   └── tools.rs        // REA-specific: ReaClient with 22 typed tool wrappers
└── Cargo.toml
```

### Usage

```rust
use mch_rea::{McpClient, McpClientConfig, ReaClient, tool_names};

let mut client = McpClient::new(McpClientConfig::default());
client.connect_stdio("rea", &[]).await?;

let rea = ReaClient::new(client);
let handle = rea.open_binary("/path/to/target.elf", None).await?;

// Full analysis pipeline
let pseudocode = rea.decompile_function(&handle, "0x401000").await?;
let call_graph = rea.get_call_graph(&handle, "0x401000", Some(3)).await?;
let xrefs = rea.find_xrefs(&handle, "malloc").await?;

// Export forensic snapshot
rea.export_evidence_bundle(&handle, "/evidence/target.rea.zip").await?;
```

---

## Key Design Decisions (For McHarness)

1. **Client, not server.** mch-rea connects to REA MCP. The heavy lifting (Ghidra, Frida, IDA) stays in REA's Node.js process. Rust stays lightweight.

2. **Handle-based API.** All tools take a `handle: String`. No binary data crosses the MCP boundary except via evidence bundles.

3. **Typed tool wrappers.** `ReaClient` provides `open_binary`, `decompile_function`, etc. as async Rust methods returning `Result<Value, McpClientError>`. No manual JSON-RPC construction.

4. **Dual transport.** `connect_stdio` for local REA (default). `connect_websocket` for remote REA (e.g., REA running on a beefy analysis box).

5. **Evidence bundles as artifacts.** `export_evidence_bundle` produces a portable `.rea.zip` that can be stored, shared, or re-imported. This is the *durable output* of RE work.

6. **Notifications.** `McpClient::notifications()` returns a stream of `McpNotification` (progress, logs, roots changed). Enables live UI updates.

---

## Integration Points

| McHarness Crate | Integration |
|-----------------|-------------|
| `mch-loop` | Drive REA tools in autonomous loops (probe → analyze → validate) |
| `mch-brain` | Store evidence bundles, function summaries, xref graphs as semantic/episodic nodes |
| `mch-mcp` | REA is an MCP server; mch-mcp can route to it like any other |
| `mch-scope` | Gate dangerous tools (Frida inject, process attach) behind approval |
| `mch-audit` | Log every REA tool call with input/output for reproducibility |

---

## Extracted Patterns for McHarness Core

| Pattern | Source | McHarness Application |
|---------|--------|----------------------|
| Handle-based resource lifecycle | REA binary handles | `mch-snapshot` artifact handles |
| Evidence bundles (immutable, versioned) | REA `.rea.zip` | `mch-snapshot` + `mch-brain` memory export |
| Probe → Analyze → Generate → Validate loop | REA recipes / DSH workflow | `mch-loop` autonomous RE pipeline |
| Tool registry with capability discovery | MCP `tools/list` | `mch-mcp` capability negotiation |
| Native parsers for fast triage | REA TS parsers | `mch-rea` could embed goblin/pe-parse for offline triage |
| Frida session as MCP resource | REA Frida tools | `mch-browser` dynamic analysis pattern |

---

## Open Questions / Future Work

- [ ] **Embedded fast triage:** Add `goblin`/`pe-parse`/`object` to mch-rea for zero-RTT ELF/PE inspection without REA server.
- [ ] **Streaming decompilation:** Large functions → stream pseudocode chunks via MCP notifications.
- [ ] **Binary diffing:** `compare_bundles` tool → integrate with `mch-audit` for regression detection.
- [ ] **Symbol propagation:** `set_address_name` → auto-sync to `mch-brain` semantic nodes.
- [ ] **Headless CI mode:** REA + mch-rea in GitHub Actions for automated binary analysis on PR.

---

## Files Reference

| File | Description |
|------|-------------|
| `forge/rea/src/application/reverse/ReverseEngineeringService.ts` | Orchestration service |
| `forge/rea/src/native/` | Pure TS binary parsers |
| `forge/rea/src/composition/reverseEngineering.ts` | RE recipes |
| `forge/rea/src/contracts/` | Zod schemas (tool I/O) |
| `crates/mch-rea/src/protocol.rs` | MCP protocol types |
| `crates/mch-rea/src/transport.rs` | Stdio + WebSocket transports |
| `crates/mch-rea/src/client.rs` | MCP client implementation |
| `crates/mch-rea/src/tools.rs` | REA-specific typed wrappers |
| `docs/ingestor/rea.md` | This document |