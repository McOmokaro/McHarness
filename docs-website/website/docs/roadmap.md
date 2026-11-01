---
title: Roadmap
sidebar_label: Roadmap
sidebar_position: 5
slug: /roadmap
description: What is built in McHarness, what is forged, and what is next.
---

# Roadmap

McHarness is built in a fixed order, and everything else compounds on it. This page is the living record of what is set, what is forged, and what is next.

## Build order

The harness is assembled one load-bearing piece at a time:

1. **Repository** — the private repo, the blueprint, and the Forge Ledger.
2. **Skeleton** — the Rust core: the agent loop, three-strata memory, and the model layer.
3. **The Key Farm** — pooled keys, rate-limit awareness, and leaderboard fallback.
4. **Fusion, Fission & the Ingestor** — the unified skill and plugin cores.
5. **The TUI** — the interface: live key-farm telemetry and model-routing visualization.
6. **The web front door** — the same core, reachable from any phone or browser.

:::note
The front door is a **gateway**, not the core. The native agent runs on your device for speed and privacy; the web layer only lets you reach it from anywhere.
:::

## Foundation

- [x] Private GitHub repository born with README, license, and the Forge Ledger
- [x] Release pipeline staged — every tag produces downloadable archives
- [x] Rust core skeleton — plan-act-observe loop, three-strata memory, model-agnostic layer
- [x] C++ speed organs scaffolded behind a safe FFI boundary
- [x] Python bridge for research and ingestion
- [ ] Key Farm — pooled keys, live rate-limit telemetry, leaderboard failover
- [ ] Fusion and Fission — unified, domain-addressed skill and plugin cores
- [ ] The Ingestor — one-command ingestion of any open-source agent
- [ ] The TUI — the full interface

## The Forge campaign

Every capability is studied, extracted, and re-forged with attribution. Status follows the Ledger taxonomy: `studied → extracting → forged → merged`.

| Lineage | Mine | Status |
|---|---|---|
| OpenCode | Streaming step loop, in-flight snapshots, failure taxonomy | Extracting |
| Hexstrike | Offensive-tooling orchestration | Extracting |
| Bug-bounty / pentest agents | Scope-safety, completion gates, exhaustion taxonomy | Extracting |
| OpenWorker | Self-waking turns, pre-spawn claiming, attendance model | Extracting |
| Fabric (Miessler) | Pattern directory, prompt-assembly grammar | Extracting |
| Hermes | Skill-sealing, the learning loop, deep memory | Studied |
| Goose, Paperclip, Pi, Deerflow, Mercury, OpenClaw, N8N, Open Design | MCP tooling, orchestration, budgets, approvals | Studied |

## Core crates forged so far

The consensus jewels mined from the ledger became the first real modules of the harness:

- [x] `mch-scope` — fail-closed scope authority and the pre-execution interceptor
- [x] `mch-gate` — machine-checkable completion gates
- [x] `mch-ledger` — the technique ledger with status taxonomy and crash-safe persistence
- [x] `mch-scheduler` — pre-spawn claiming and the injected runner
- [ ] `mch-clock`, `mch-compaction`, `mch-audit` — cadence, context overflow, and the event bus

## Releases

| Version | Name | Contents |
|---|---|---|
| `v0.0.1` | Skeleton | Repository, Rust core, C++ organs, Python bridge, Forge Ledger |

See [The Forge](/the-forge) for how each lineage is extracted, and [Architecture](/architecture) for where the forged code lives.
