# Ingestor Dossier — OpenCode

Lineage: OpenCode (SST / Anomaly Innovations). MIT-licensed terminal AI coding agent.
Status: **extracting** — study complete, re-implementation plan drafted.

Method: source-read, not hearsay. Every claim below was verified against the actual repository (`packages/core/src`), not documentation.

## What OpenCode actually is (verified from source)

- Client/server: a persistent server owns session state and provider traffic; the TUI, desktop app, and IDE extensions are all just clients. Sessions survive terminal restarts and reconnects.
- Two built-in agent modes — `build` (full access) and `plan` (read-only, bash needs permission) — switched with one keypress, plus a `general` subagent for multi-step searches.
- Core written in TypeScript on the Effect runtime; everything is a Service with typed dependencies.

## Crown jewels mined

### 1. The streaming step loop (the deepest find)

`session/runner/step.ts` is the heart. The step loop does NOT wait for the model to finish before acting:

- The provider response is consumed as a **live stream**. The moment a tool-call event arrives, its execution is forked onto a concurrent fiber — while the model is still generating the next tokens.
- Provider events and tool executions keep **per-source ordering** with no shared writer queue. Tools run truly in parallel; results publish in arrival order.
- This is why OpenCode feels light-speed: tool latency is overlapped with generation latency.

**→ Re-implementation in McHarness (Rust):** the step loop becomes an async pipeline — one task streams the provider, spawns `tokio::spawn` per tool call the instant the call is parsed, results drain through a bounded channel with per-source ordering preserved. The user never sees a tool "pause" the model.

### 2. Snapshot-while-requesting

A filesystem snapshot is captured **forked while the provider request is in flight** — never delaying the request, but always present before any local tool runs (local tools cannot run before `Step.Started` publishes). Free rollback points with zero latency cost.

**→ Re-implementation:** snapshot task forked at step start in Rust; joined lazily only when a mutating tool first fires.

### 3. Overflow and failure taxonomy

The runner classifies failures instead of crashing:

- **Context overflow detected before any output started** → trigger compaction, retry the step. Detected from the stream itself (`isContextOverflowFailure`), not a blind retry.
- **Transport failure with zero output started** → `RecoverFull`: replay the whole step from scratch.
- **Retryable vs. fatal** decided from recorded stream state (`outputStarted`, finish reason) — an interrupted stream after partial output is handled differently from a clean failure.
- Reading continues to the end of the stream even past errors, so the next request can reuse the response.

**→ Re-implementation:** the same taxonomy becomes McHarness's error state machine, and it composes with the Key Farm: a retryable provider failure is first a *reroute* opportunity (another key/model), then a retry.

### 4. The max-steps guard

When step budget is exhausted, tools are disabled and a strict injected prompt (`MAX_STEPS_PROMPT`) forces a text-only summary: what was done, what remains, what to do next. No runaway loops.

**→ Re-implementation:** identical guard in the Rust loop; the summary text is persisted to memory so the next session picks up the thread.

### 5. Permission gating as a service

Permissions are not scattered if-statements — they are a Service (`Permission`). Tool executions raise `Permission.DeclinedError` through the same fiber machinery as every other outcome. Question prompts (ask-the-user tools) integrate through the same channel.

**→ Re-implementation:** a permission engine in the Rust core; every tool declares its permission class; approvals flow through the same event bus as progress.

### 6. Session event bus

Everything — provider events, tool progress, step lifecycle — publishes through one event bus (`Bus`). Clients subscribe; the server never blocks on a slow client.

**→ Re-implementation:** McHarness's bus becomes the single spine: the same events drive the terminal UI, the future web front door, and the memory strata.

### 7. Skill + instruction discovery

Skills are first-class (`skill.ts`, `skill/`); instruction files (AGENTS.md) are discovered recursively (`instruction-discovery.ts`), not hardcoded. Model routing is data-driven via the models.dev catalog (`models-dev.ts`, `model-resolver.ts`).

**→ Re-implementation:** the skill loader and instruction discovery feed Fusion directly; the model resolver pattern is absorbed into the Key Farm's leaderboard fallback.

## What we deliberately do NOT copy

- The Effect/TypeScript service scaffolding — McHarness's core is Rust; we take the *patterns*, not the runtime.
- SQLite-as-session-store as-is — McHarness's three-strata memory replaces it with its own design.

## Forge actions queued

1. `whole/loop.rs` — streaming step pipeline with forked tool fibers (jewel 1, 4)
2. `whole/snapshot.rs` — in-flight snapshot capture (jewel 2)
3. `whole/failure.rs` — overflow/retry/reroute taxonomy wired to Key Farm (jewel 3)
4. `whole/permissions.rs` — permission engine (jewel 5)
5. `bus/` — event spine shared by UI, web door, memory (jewel 6)
6. `fusion/discovery.rs` — skill + instruction discovery (jewel 7)

Next lineage after OpenCode extraction: **Hermes** (skill-sealing, learning loop, deep memory).
