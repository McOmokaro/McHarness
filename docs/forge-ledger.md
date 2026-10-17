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
| **Mercury** | extracting | two-lane argv/shell permissioning with `$(...)`-laundering defeat and pinned-PATH ShellCheck lanes, fstat `{dev,ino}` TOCTOU defense for FS access, soul-file identity with drift-guarded migration, daily-reset token budget with stable-prefix cache-friendly prompt assembly, Jaccard tool-call loop detection, work-ledger with interrupt-recovery semantics, forced-action rounds (`toolChoice: 'required'`) — see [docs/ingestor/mercury.md](ingestor/mercury.md) |
| **OpenClaw** | extracting | respawn-to-health entry chain, phased gateway turn pipeline with composable admission-currency assertions, two-level (global/session) run lanes with epoch tokens, prepared-model-runtime leases with arc-swap generation swaps, gateway-lock + abort-signaled state ownership (port 18789), channels-as-plugins with debounce/mention-gating/allowlists, `safeBins`/`safeBinTrustedDirs` exec allowlists, heartbeat as undeletable system-owned cron declaration, SQLite state with dedupe replay cache, provenance-tagged writes — see [docs/ingestor/openclaw.md](ingestor/openclaw.md) |
| **N8N** | extracting | execution-stack dataflow scheduler (arrival-driven, no topo-sort), canvas-position deterministic ordering, pinned outputs as cache-as-control-flow, json-error sentinel + retryOnFail, EngineRequest/EngineResponse suspendable-coroutine agent loop, requiredInputs join barrier, leadership-gated cron, hook-driven lifecycle event spine — see [docs/ingestor/n8n.md](ingestor/n8n.md) |
| **Open Design** | extracting | declarative runtime adapter defs (28 CLIs as data), resume-invalidation guard (cursor = last assistant message, drift forces reseed), tool-loop guard with mutating-vs-read-only progress semantics, host-evidenced turn completion, cache-ordered prompt composer, run-diff content-addressed file versioning, sandbox env redirection jail, opaque sync-digest tokens — see [docs/ingestor/open-design.md](ingestor/open-design.md) |

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
