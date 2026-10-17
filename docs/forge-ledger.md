# Forge Ledger

The public map of McHarness's lineages: which capability was mined from which agent, and what became of it.

Method: **study, re-implement, attribute.** No wholesale copying — every lineage is honored by native re-implementation, keeping McHarness original work standing on giants.

## Harness Lineages

| Lineage | Status | Crown jewels mined |
|---|---|---|
| **OpenCode** | extracting | streaming step loop (tools fork while the model still generates), in-flight snapshots, overflow/retry taxonomy, max-steps guard, permission engine, event bus, skill discovery — see [docs/ingestor/opencode.md](ingestor/opencode.md) |
| **Hermes** | extracting | marker-gated incremental FTS memory backfill, cache-boundary prompt registry, compaction summary preamble as contract, contextvar route pinning, fingerprinted memory ids, failure-shape classifiers, skill sealing + learning loop — see [docs/ingestor/hermes.md](ingestor/hermes.md) |
| **Goose** | extracting | persisted-conversation state machine, pause-on-confirmation turn loop, first-answer-wins confirmations, layered deny-by-default permission inspectors, injection-safe LLM judge, retry classification as data, namespacing with mangle-recovery, interrupted-turn hygiene — see [docs/ingestor/goose.md](ingestor/goose.md) |
| **Pi** | extracting | config-callback loop with two queue drain points, replayable transcript deltas, tree-structured JSONL sessions, truncation-safe tool batches, structuredContent/details tool split, unanimous-terminate, mode-uniform core — see [docs/ingestor/pi.md](ingestor/pi.md) |
| **Paperclip** | extracting | DB-backed wakeup queue with coalescing + idempotency fences, atomic task checkout, typed waiting postures, "narration is not a wake path" disposition guard, bounded deterministic handoff, budget reservations + versioned delivery, run-scoped JWT attribution, skill-as-operating-manual — see [docs/ingestor/paperclip.md](ingestor/paperclip.md) |
| **Deerflow** | extracting | middleware-as-architecture with anchored insertion, turn-budget translation (middleware-aware recursion limits), immutable parent snapshots for sub-agent isolation, first-writer-wins terminal state, fail-closed tri-state capability flags, pluggability-by-construction invariants, compaction-refuses-unsafe-states, receipts that survive compaction — see [docs/ingestor/deerflow.md](ingestor/deerflow.md) |
| **Mercury** | studied | diffusion-style speed |
| **OpenClaw** | studied | autonomy scaffolding |
| **N8N** | studied | workflow graph model |
| **Open Design** | studied | design-generation pipeline |

## Master-Bot Lineage

| Lineage | Status | Notes |
|---|---|---|
| **grok bot** | studied | merged into the master bot |
| **muse** | studied | merged into the master bot |
| **cue** | studied | merged into the master bot |

McHarness itself is the fusion of all bots — the master bot.

## Skill & Pattern Lineages

| Lineage | Status | Notes |
|---|---|---|
| **Daniel Miessler / Fabric** | extracting | lifeos, personas, decision frameworks → Fusion core |
| **Fusion corpus** | forging | all skills ever released, reverse-engineered into one core |
| **Fission corpus** | forging | all plugins as one, same pattern as Fusion |

## Security Team Lineages (scope-safety bound)

| Lineage | Status | Notes |
|---|---|---|
| **Hexstrike** | studied | offensive orchestration — authorized targets only |
| **Bug-bounty agents** | studied | target discovery, reporting — authorized targets only |
| **OpenWorker** | studied | execution & delivery — authorized targets only |

The security team acts **only on explicitly authorized targets**. Scope safety is enforced at the subsystem level, not by request.
