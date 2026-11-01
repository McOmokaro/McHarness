---
title: Architecture
sidebar_label: Architecture
sidebar_position: 3
slug: /architecture
description: The McHarness language stack and core subsystems — Rust for the loop, Go for the Key Farm, C++ for speed, Python for research.
---

# Architecture

McHarness is one harness, written in four languages, each chosen for the work it does best. This page explains the split, the reasons behind it, and the subsystems that make the **Whole** possible.

## The language stack

| Layer | Language | Share | Responsibility |
|---|---|---|---|
| **The Loop** | Rust | 60% | The agent loop, three-strata memory, the model connection layer, and the orchestration core |
| **The Key Farm & Tooling Bridge** | Go | 15% | The key pool, provider routing, and the bridge to external tooling (including Kali-compatible security tools) |
| **The Speed Organs** | C++ | 15% | Token streaming, fast memory search, embedding and similarity math |
| **Research & Ingestion** | Python | 10% | Data-collection and enrichment pipelines, OSM/open-data mining, skill prototyping, and Fabric-pattern ingestion |

Each layer talks to the next across a **safe boundary**: Rust is the body and nervous system, C++ is the raw muscle, Go is the connective tissue for keys and external tools, and Python is the lab where new capabilities are prototyped before they are promoted into the compiled core.

## The Loop (Rust — 60%)

The Rust core is where the **Whole** design law is enforced. It owns:

- **The agent loop** — goal in, plan, act, observe, loop. The same fundamental loop the best harnesses are built on, re-implemented from first principles.
- **Three-strata memory**, compiled in as a native subsystem rather than a prompt history:
  - **Persistent memory** — durable facts that survive across sessions.
  - **Working memory** — the live state of the current task.
  - **Skill memory** — the procedures and triggers of the compiled-in skills.
- **The model connection layer** — talks to local models and hosted APIs alike, with automatic fallback handled in coordination with the Key Farm.
- **The orchestration core** — the Cargo workspace that binds every module together.

The core is built as a Cargo workspace, with sharp, single-purpose crates:

| Crate | Responsibility |
|---|---|
| `mch-core` | The binary and the agent loop that ties the system together |
| `mch-scope` | Fail-closed scope authority and the pre-execution interceptor |
| `mch-gate` | Machine-checkable completion gates |
| `mch-ledger` | The technique ledger with its status taxonomy and crash-safe persistence |
| `mch-scheduler` | Task scheduling: pre-spawn claiming and the injected runner |
| `mch-clock` | Time and cadence — deadlines, reporting rhythm, self-waking turns |
| `mch-compaction` | Context-overflow handling and retry |
| `mch-audit` | The event bus and the audit trail |

:::note
The first three crates — `mch-scope`, `mch-gate`, and `mch-ledger` — did not start life as general infrastructure. They are the direct descendants of the best scope-safety model found during the Forge campaign, promoted from a security concern into the foundational **execution-governance layer** of the whole harness. See [The Forge](/the-forge).
:::

## The Key Farm & tooling bridge (Go — 15%)

The **Key Farm** is the flagship differentiator, and it lives in Go for its concurrency model and its tooling ecosystem.

- **One pool of keys.** You name the model you want per task; the Farm finds every key that carries it and lines them up as automatic fallbacks.
- **Rate-limit awareness.** A live panel tracks key health and pauses or reroutes *before* a request fails.
- **Leaderboard-driven failover.** When a model is unavailable across every key, the Farm consults the top model leaderboards — per-domain (for example, design) and general — and selects the next best proven model for that task.
- **The tooling bridge.** Go is also the compatibility layer for external and offensive tooling, including **Kali-compatible security tools**, so the Cybersecurity team can drive real tools without dragging a scripting runtime into the hot path.

## The Speed Organs (C++ — 15%)

The C++ layer is where McHarness earns its speed advantage over Python-only agents. Rust reaches into these modules through a safe FFI boundary. They handle:

- **Token streaming** — the bytes flow the instant the model produces them.
- **Fast memory search** — searching across the three memory strata at speed.
- **Embedding & similarity math** — the vector work behind memory recall and skill matching.

These are the "light-speed organs" behind the **Whole** promise: tool use fast enough that it stops feeling like tool-calling.

## Research & Ingestion (Python — 10%)

Python is where McHarness is *built*, not where it *runs* in the hot path. It owns:

- **Data-collection and enrichment pipelines** — for example, OpenStreetMap and open-data lead mining for high-demand regions.
- **Skill prototyping** — new skills are developed in Python, then promoted into the compiled core once proven.
- **Fabric-pattern ingestion** — the Daniel Miessler lineage (patterns, lifeOS, personas, decision frameworks) is mined and folded into Fusion's skill core.
- **The Ingestor's heavy lifting** — the analysis tooling that turns a cloned repository into a dossier.

## How it fits together

```text
        ┌──────────────────────────── Rust core (60%) ────────────────────────────┐
        │  agent loop · memory strata · model layer · orchestration               │
        └───────┬─────────────────────────┬──────────────────────────┬────────────┘
                │ safe FFI                │ control                  │ bridge
        ┌───────▼────────┐        ┌───────▼────────┐         ┌───────▼────────┐
        │  C++ (15%)     │        │  Go (15%)      │         │  Python (10%)  │
        │  speed organs  │        │  Key Farm +    │         │  research +    │
        │  streaming,    │        │  tooling bridge│         │  ingestion +   │
        │  search, matmul│        │  (Kali compat) │         │  prototyping   │
        └────────────────┘        └────────────────┘         └────────────────┘
```

## Design constraints

1. **Whole, not glued.** Tools, memory, skills, and inference are compiled-in subsystems — no visible seams between intent and action.
2. **Featherweight.** Capabilities are lazy placeholders that install only on first use (the Omarchy model).
3. **Model-agnostic from birth.** Any provider, any key, any model.
4. **Fail-closed by default.** When scope or a completion gate is uncertain, McHarness stops rather than proceeds.
5. **Attributed.** Every forged capability is traced in the Forge Ledger.

## Where to go next

- **[The Forge](/the-forge)** — how each capability was reverse-engineered and recorded
- **[Quickstart](/quickstart)** — install and initialize the Key Farm
