---
title: McHarness
sidebar_label: Overview
sidebar_position: 1
slug: /
description: McHarness — the Father of Harnesses. One whole, light-speed, featherweight multi-agent terminal harness, forged by reverse-engineering the best open-source agents.
---

# McHarness

**The Father of Harnesses** — one whole, light-speed, featherweight multi-agent terminal harness.

McHarness is not a framework you bolt together from a dozen packages, and it is not a chatbot with a plugin folder. It is a **single native agent** that runs in your terminal, installs with one command, and carries its tools, memory, and skills compiled deep inside it. Every capability it has was studied from the best open-source agents in the ecosystem and re-forged natively — with the lineage recorded, in public, in the Forge Ledger.

This documentation covers **what McHarness is**, **how to install it**, **how it is architected**, and **how the Forge works**.

## What McHarness is

- **A native terminal agent.** You run it in a terminal, it lives on your machine, and people install it with a single `curl` command from GitHub. No web app, no hosted database, no VPS required.
- **A synthesis, not a wrapper.** Its capabilities were reverse-engineered from the best open-source harnesses — OpenCode, Hermes, Goose, Paperclip, Pi, and more — and re-implemented from first principles in one codebase.
- **Deeply model-agnostic.** Bring any provider, any key, any model. McHarness never locks you to one vendor.
- **Open source.** The Forge Ledger is public proof that McHarness is built from the best of all of them, with attribution.

## The "Whole" philosophy

The central design law of McHarness is **Whole**. In today's agents, every action is a separate handshake: a tool call here, a script there, a memory layer that is really just a pile of old prompts. McHarness rejects that.

> Think of the accelerator pedal in a car. You press it, and the car simply accelerates — you do not think about fuel injection, gears, or combustion. McHarness is built the same way: **intent in, action out, no visible seams.**

Tools, memory, skills, and model inference are compiled into **one organism** rather than wired together as external call chains. The result is a system where tool use happens at light speed and low latency — fast enough that it stops feeling like tool-calling at all, and starts feeling like a whole.

:::info Why it matters
The "Whole" law is why McHarness spends its performance budget the way it does: hot paths live in C++, memory and skills are native subsystems, and there is no interpreter in the loop between your intent and the machine's action.
:::

## Featherweight by design — the Omarchy approach

You would expect a "father of harnesses" to weigh gigabytes. It does not.

McHarness follows the **Omarchy philosophy**: capabilities are **placeholders that install themselves only at the moment they are actually needed**. The base download stays tiny, and the agent grows only where you use it. A tool you never touch never lands on disk; a capability you invoke once is fetched on first use and then cached.

This is what lets a single agent carry the best capabilities of a dozen ecosystems without carrying a dozen ecosystems' worth of bloat.

## The subsystems

| Subsystem | What it does |
|---|---|
| **Fusion** | The unified **skill** core. Every reverse-engineered skill is compiled in and addressed by **domain, not command syntax**. Ask a question; the relevant skills are already armed. |
| **Fission** | The **plugin** core. The same unified, domain-addressed, one-command-update model — for plugins. |
| **The Key Farm** | All your API keys act as **one pool**. Name the model you want per task; the Farm finds every key that carries it, lines them up as automatic fallbacks, and reroutes before a rate limit becomes a wall. |
| **The Ingestor** | Drop in **any** open-source agent. McHarness studies it, mines its crown jewels, and weaves them into the ecosystem. |
| **The Forge Ledger** | The public **provenance map** — every capability traced to the agent it was forged from, with a status of `studied → extracting → forged → merged`. |
| **Three-strata memory** | **Persistent**, **working**, and **skill** memory compiled into the core. Not a prompt history. |
| **The TUI** | The terminal interface: live key-farm telemetry, model-routing visualization, and memory and skill activity — dark, fast, alive. |

## First specialist unit: the Cybersecurity team

The first specialist team forged into McHarness is the **Cybersecurity room**, assembled from three lineages and governed by a non-negotiable rule: **the team only ever acts on targets you explicitly authorize.**

| Lineage | Forged capability |
|---|---|
| **Hexstrike** | Offensive-tooling orchestration |
| **Bug-bounty / pentest agents** | Target discovery, reporting, and the strongest scope-safety model in the ledger |
| **OpenWorker** | Task execution and delivery |

Because the mined offensive orchestrators shipped with unenforceable scope safety, McHarness supplies its own: fail-closed scope authority, machine-checkable completion gates, and an auditable technique ledger. See [The Forge](/the-forge) for how each lineage was extracted.

## The language stack

McHarness is written across four languages, each chosen for what it does best. See [Architecture](/architecture) for the full breakdown, but in one line: **Rust** for the loop, **Go** for the Key Farm and tooling bridge, **C++** for the speed organs, and **Python** for research and ingestion.

## Status

McHarness is under active development. The skeleton is set and the Forge campaign is underway — the harness boots, runs the plan-act-observe loop, and carries its first forged modules.

## Where to go next

- **[Quickstart](/quickstart)** — install McHarness and initialize the Key Farm
- **[Architecture](/architecture)** — the language stack and the core subsystems
- **[The Forge](/the-forge)** — the Ingestor, the Forge Ledger, and how lineages are woven in
- **[Roadmap](/roadmap)** — what is built and what is next
