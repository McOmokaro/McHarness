---
title: The Forge
sidebar_label: The Forge
sidebar_position: 4
slug: /the-forge
description: The Ingestor, the Forge Ledger, and how McHarness reverse-engineers OpenCode, Hermes, and the cybersecurity agents into one harness.
---

# The Forge

Every capability in McHarness was **studied, extracted, and re-forged** from the best open-source agents in the ecosystem. This page explains the two machines behind that process — the **Ingestor** and the **Forge Ledger** — and walks through the lineages already mined.

## The Ingestor

The **Ingestor** is the front door of the Forge. You hand it **any** open-source agent, and it:

1. **Clones** the repository and reads it source-deep.
2. **Mines** the crown jewels — the one or two capabilities actually worth having.
3. **Re-implements** them natively in the McHarness stack (Rust, Go, C++, or Python).
4. **Records** the lineage in the Forge Ledger.

Hermes, OpenCode, Goose, Pi, Paperclip — and the next unknown agent released next month — all become McHarness the same way.

## The Forge Ledger

The **Forge Ledger** is the public provenance map. It is what makes the claim *"built from the best of all of them"* auditable rather than aspirational: every capability is traced to the agent it came from, with an explicit status.

| Status | Meaning |
|---|---|
| `studied` | The repository has been read and understood |
| `extracting` | Crown jewels identified; re-implementation in progress |
| `forged` | Re-implemented natively and passing tests |
| `merged` | Live in a McHarness release |

## The method (and the legal discipline)

Because the source harnesses are MIT- and Apache-licensed, the Forge follows one rule without exception:

> **Study and re-implement with attribution — never copy-paste wholesale.**

This keeps McHarness legally clean as original work, which matters for the "Father of Harnesses" brand. The blueprint is not their code; it is the *idea* of their code, rebuilt from first principles and credited in the Ledger.

## Lineages already forged

### OpenCode — the coding loop

The first lineage. Its crown jewels were verified from source, not from marketing:

- **The streaming step loop** — each tool is forked onto a concurrent fiber *the instant the call arrives*, while the model is still generating. This is the secret of its light-speed feel, and it became the heart of the Rust core's loop.
- **In-flight snapshots** — filesystem snapshots captured in the background during a model request, never delaying it.
- **A failure taxonomy** — context overflow triggers compaction and retry; clean transport failures replay the step. This wires directly into the Key Farm: a retryable failure is a reroute opportunity first.
- **Max-steps guard, permission engine, event bus, and skill discovery** — all mined into the core.

### Hermes — skill-sealing, learning loop, and memory

Hermes is notable for its **learning loop**: it writes reusable skills from its own runs. McHarness makes that pattern native:

- **Skill-sealing** — a completed run can be sealed into a reusable, compiled-in skill.
- **The learning loop** — the agent improves its skill core over time from real use.
- **Deep memory** — the three-strata memory model (persistent, working, skill) traces its inspiration here and is compiled into the Rust core rather than bolted on.

### The Cybersecurity team

The first specialist unit, assembled from three lineages:

- **Hexstrike** — offensive-tooling orchestration. Mined for its two-process offense architecture and its timeout-partial-results executor. Crucially, the study produced a sober verdict: **its scope-safety is unenforceable at the boundary**, so McHarness must supply its own.
- **Bug-bounty / pentest agents** — the strongest scope-safety lineage in the entire ledger: **placeholder-scope refusal** ("incomplete scope = hard stop"), **fail-closed three-state verdicts**, **machine-checkable completion gates**, and a structured brain with a **12-status exhaustion taxonomy**. These became the `mch-scope`, `mch-gate`, and `mch-ledger` crates of the core.
- **OpenWorker** — execution and delivery: the **self-waking turn engine**, **pre-spawn task claiming**, **standing approvals**, and an **attendance model** that auto-denies when unattended.

The rule that governs the whole team is absolute: **the Cybersecurity team only ever acts on targets you explicitly authorize.**

### Fabric — patterns and prompts

The Daniel Miessler lineage: the **pattern-directory model**, the template-stack scanner, the prompt-assembly grammar, and the pattern-quality rubric. Folded into Fusion's skill core.

## Beyond the harness stack

The Forge does not stop at coding agents. The ledger also seeds:

- The **master-bot fusion** — the general-purpose bots (grok bot, muse, cue, and others) that merge into McHarness itself.
- The **Miessler line** — lifeOS, personas, and decision frameworks.
- **Future Ingestor entries** — anything you drop in, at any time.

## See the dossiers

Each lineage has a full source autopsy in the repository at `docs/ingestor/` — with exact file paths, verified references, and one-line re-implementation targets. The ledger itself lives at `docs/forge-ledger.md`.

## Where to go next

- **[Architecture](/architecture)** — where the forged capabilities live in the stack
- **[Roadmap](/roadmap)** — what has been forged so far and what is next
