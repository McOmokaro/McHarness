# OpenCode Ingestor Dossier

**Lineage:** OpenCode (anomalyco/opencode) — MIT Licensed
**Status:** `extracting` → `forged` (this dossier)
**Forge Target:** `mch-core` (Rust streaming agent loop)

---

## Executive Summary

OpenCode is a Bun/TypeScript terminal agent built on **Effect** (structured concurrency) and **@ai-sdk** (model abstraction). Its crown jewel is the **streaming step loop** — tools are forked onto concurrent fibers *the instant the call arrives*, while the model is still generating. This is the secret of its light-speed feel.

This dossier extracts seven verified crown jewels with file paths, then provides one-line Rust re-implementation targets for McHarness.

---

## Crown Jewel 1: The Streaming Step Loop (Concurrent Tool Forking)

**Location:** `packages/core/src/session/runner/step.ts:73-138`

### How it works

```typescript
const toolRuns: Array<{
  readonly call: ToolCall
  readonly fiber: Fiber.Fiber<void, Permission.DeclinedError | QuestionTool.CancelledError>
}> = []

const executeTool = (call: ToolCall) => { ... }

// Provider stream processing
const providerStream = llm.stream(input.prepared.request, input.prepared.options).pipe(
  Stream.runForEach((event) =>
    Effect.gen(function* () {
      // ... publish event ...
      if (event.type !== "tool-call" || event.providerExecuted) return
      toolRuns.push({
        call: event,
        fiber: yield* Effect.uninterruptibleMask((restore) =>
          restore(executeTool(event)).pipe(
            Effect.flatMap(toolOutput.truncate),
            Effect.flatMap((outcome) => publisher.toolExecution(event.id, event.name, outcome)),
          ).pipe(Effect.forkScoped),  // ← FORKS TOOL CONCURRENTLY
        ),
      })
    })),
  Effect.ensuring(publisher.flush()),
)
```

**Key insight:** Tools don't wait for the model to finish. As each `tool-call` event arrives from the streaming provider, the tool is *immediately* spawned on a new Effect fiber (`Effect.forkScoped`). The model keeps streaming; tools run in parallel.

### McHarness Rust Target

> **`mch-core/src/loop.rs::StepExecutor::spawn_tool_concurrently()`** — Spawn a Tokio task for each tool call the instant the `ToolCall` event is received from the streaming provider, while the model stream continues. Return a `JoinHandle` stored in a `Vec` for later join.

---

## Crown Jewel 2: In-Flight Filesystem Snapshots

**Location:** `packages/core/src/session/runner/step.ts:74-76`, `packages/core/src/snapshot.ts:128-143`

### How it works

```typescript
// The start snapshot only has to exist before local tools run,
// so it is captured while the provider request is in flight instead of delaying it.
const pendingStartSnapshot = yield* snapshots.capture().pipe(Effect.forkScoped)
```

The snapshot capture (Git tree capture) is *forked* into the background **before** the model request completes. It runs in parallel with the provider stream. The fiber is joined later when step settlement needs the snapshot.

```typescript
// Snapshot.capture()
const capture = Effect.fn("Snapshot.capture")(function* () {
  if (!enabled()) return undefined
  return yield* Effect.gen(function* () {
    const repo = yield* repository
    const tree = yield* git.tree.capture({
      repository: repo.snapshotRepository,
      scopes: [yield* scope(repo.worktree)],
      ignores: repo.source,
      maximumUntrackedFileBytes: 2 * 1024 * 1024,
    })
    yield* packWhenDue(repo.snapshotRepository)
    return ID.make(tree)
  }).pipe(Effect.catch((cause) => Effect.logWarning(...).pipe(Effect.as(undefined))))
})
```

### McHarness Rust Target

> **`mch-core/src/snapshot.rs::SnapshotService::capture_in_flight()`** — Spawn a background Tokio task that captures a Git tree snapshot (using `git2` or `libgit2`) while the model request is in flight. Return a `oneshot::Receiver<SnapshotId>` for later await.

---

## Crown Jewel 3: Failure Taxonomy (Context Overflow → Compaction, Transport → Retry)

**Location:** `packages/core/src/session/runner/step.ts:113-119, 160-171, 186-203, 266-274`

### How it works

OpenCode classifies provider failures into a precise taxonomy and routes each differently:

```typescript
// 1. CONTEXT OVERFLOW → Compaction (not retry)
if (LLMEvent.is.providerError(event) &&
    isContextOverflowFailure(event) &&
    !publisher.record().outputStarted) {
  overflowFailure = event
  return  // Don't publish; wait for compaction
}

// Later in settlement:
if (!publisher.record().outputStarted &&
    isContextOverflowFailure(overflow) &&
    (yield* restore(input.recoverOverflow))) {
  return Outcome.Compacted()  // Trigger compaction, then retry step
}

// 2. TRANSPORT FAILURE (clean) → Retry with continuation
if (llmFailure?.reason._tag === "Transport" &&
    (llmFailure.reason.recovery === "retry-full" || llmFailure.reason.recovery === "rotate-and-retry-full") &&
    !recorded.outputStarted)
  return Outcome.RecoverFull()  // Full retry, same assistant message

// 3. RETRYABLE FAILURE → Retry with decision
const retry = llmFailure && llmError && !isContextOverflowFailure(llmFailure)
  ? yield* restore(input.retry(llmFailure, llmError, SessionRunnerRetry.isRetryable(llmFailure) || ...))
  : undefined

// 4. CONTINUE AFTER INCOMPLETE STREAM → Synthetic continue prompt
if (llmFailure?.reason._tag === "InvalidProviderOutput" && llmFailure.reason.classification === "incomplete-stream")
  // Injects: "The previous response was interrupted. Continue from where you left off..."
```

### McHarness Rust Target

> **`mch-core/src/failure.rs::FailureClassifier::classify()`** — Enum `FailureAction { Compact, RetryFull, RetryPartial, ContinueIncomplete, Fail }` with decision logic matching OpenCode's taxonomy. Integrates with `mch-compaction` for overflow path.

---

## Crown Jewel 4: Max-Steps Guard (Tool Disable at Limit)

**Location:** `packages/core/src/session/runner/max-steps.ts`, `packages/core/src/session/runner/llm.ts:249-269`

### How it works

```typescript
const stepLimitReached = loaded.agent.info.steps !== undefined && step >= loaded.agent.info.steps

const transcript = SessionModelRequest.baseTranscript({ ... })
const prepared = yield* context.request.primary({
  ...
  toolChoice: stepLimitReached ? "none" : undefined,  // ← DISABLES TOOLS
})
```

When the step counter hits the agent's configured max, the request is sent with `toolChoice: "none"` — the model *cannot* call tools. The `MAX_STEPS_PROMPT` is injected as a synthetic assistant message forcing a text-only summary.

### McHarness Rust Target

> **`mch-core/src/loop.rs::StepExecutor::check_max_steps()`** — Before each step, check `step_count >= agent.max_steps`. If exceeded, inject a `ToolChoice::None` into the request and append the `MAX_STEPS_PROMPT` as a synthetic message.

---

## Crown Jewel 5: Permission Engine (Ruleset Evaluation + Unattended Mode)

**Location:** `packages/core/src/permission.ts:86-100, 161-202, 248-279`

### How it works

```typescript
export function evaluate(action: string, resource: string, ...rulesets: Permission.Ruleset[]): Permission.Rule {
  return rulesets.flat().findLast((rule) =>
    Wildcard.match(action, rule.action) && Wildcard.match(resource, rule.resource)
  ) ?? { action, resource: "*", effect: "ask" }
}

// Unattended mode: when nobody is present, "ask" becomes "deny"
const unattended = yield* Unattended
if (event.effect !== "ask" || unattended === undefined)
  return { effect: event.effect, ... }
const settled = [...all, ...unattended]
const allowed = input.resources.every((resource) =>
  evaluate(input.action, resource, settled).effect === "allow"
)
if (allowed) return { effect: "allow" }
return { effect: "deny", message: "Permission needs approval, and nobody is present to give it" }
```

Rulesets: agent permissions → session permissions → saved "always" permissions → unattended fallback.

### McHarness Rust Target

> **`mch-scope/src/permission.rs::PermissionEngine::evaluate()`** — Pure function: `evaluate(action, resource, &[Ruleset]) -> Effect` with wildcard matching. `UnattendedRuleset` overlay turns `Ask` → `Deny`. Integrates with `mch-scope` pre-execution interceptor.

---

## Crown Jewel 6: Event Bus (Bus.Service — Structured Event Publication)

**Location:** `packages/core/src/bus.ts`, `packages/core/src/session/event.ts`

### How it works

```typescript
// Core bus interface
export interface Interface {
  readonly publish: <E extends SessionEvent.Type>(
    type: E,
    payload: SessionEvent.Payload<E>
  ) => Effect.Effect<void>
  readonly subscribe: <E extends SessionEvent.Type>(
    type: E,
    handler: (payload: SessionEvent.Payload<E>) => Effect.Effect<void>
  ) => Effect.Effect<Effect.Effect<void>>
}

// Events include: Step.Started, Step.Ended, Tool.Started, Tool.Completed, Tool.Failed, etc.
```

Every significant action publishes a typed event. The `publisher` in `step.ts` wraps this for step-scoped events with proper sequencing.

### McHarness Rust Target

> **`mch-core/src/events.rs::EventBus`** — Lock-free MPSC channel (`tokio::sync::broadcast`) for typed events. `publish(Event)` / `subscribe(EventType) -> Receiver`. Zero-cost when no subscribers.

---

## Crown Jewel 7: Skill Discovery (Plugin-Scanned, Permission-Filtered)

**Location:** `packages/core/src/tool.ts:265-315`, `packages/core/src/plugin/skill.ts`

### How it works

```typescript
snapshot: Effect.fn("Tool.snapshot")((permissions) =>
  Effect.sync(() => {
    const active = new Map<string, Tool.Info>()
    for (const [name, tool] of data.tools) {
      if (whollyDisabled(tool.options?.permission ?? name, rules)) continue
      active.set(name, tool)
    }
    // Code Mode tools get special handling...
    return {
      definitions: [...direct, ...codeModeDefinition],
      execute: (input) => { ... }
    }
  })
)
```

Skills are discovered from plugins, filtered by permissions at snapshot time, and compiled into `ToolDefinition` array for the model. The `execute` closure captures the session context.

### McHarness Rust Target

> **`mch-core/src/skills.rs::SkillRegistry::snapshot()`** — Returns `(definitions: Vec<ToolDefinition>, execute: Fn(ToolCall) -> Result)` filtered by `mch-scope` permissions. Skills are compiled-in modules, not dynamic plugins.

---

## Complete File Map

| Capability | OpenCode File | Lines | McHarness Target |
|------------|---------------|-------|------------------|
| Streaming step loop | `session/runner/step.ts` | 73-138 | `loop.rs::StepExecutor` |
| In-flight snapshots | `session/runner/step.ts:74` + `snapshot.ts:128` | 74-76, 128-143 | `snapshot.rs::capture_in_flight()` |
| Failure taxonomy | `session/runner/step.ts` | 113-119, 160-171, 186-203, 266-274 | `failure.rs::FailureClassifier` |
| Max-steps guard | `session/runner/llm.ts` + `max-steps.ts` | 249-269, 1-16 | `loop.rs::check_max_steps()` |
| Permission engine | `permission.ts` | 86-100, 161-202, 248-279 | `mch-scope/src/permission.rs` |
| Event bus | `bus.ts` + `session/event.ts` | all | `events.rs::EventBus` |
| Skill discovery | `tool.ts` | 265-315 | `skills.rs::SkillRegistry` |
| Model transport (WS with fallback) | `session/model-transport.ts` | 289-506 | `transport.rs::WebSocketTransport` |
| Compaction (auto + native) | `session/compaction.ts` | 292-760 | `mch-compaction` crate |
| Retry policy | `session/runner/retry.ts` | 32-55, 57-77 | `loop.rs::RetryPolicy` |

---

## Sweet Things (Copy-Pasteable Patterns)

### 1. Effect.forkScoped for Tool Concurrency
```typescript
// Spawn tool on fiber, auto-cleanup on scope exit
fiber: yield* Effect.uninterruptibleMask((restore) =>
  restore(executeTool(event)).pipe(Effect.forkScoped)
)
```
**Rust:** `tokio::task::spawn` with `JoinHandle` stored in `Vec`, cancelled on `Drop`.

### 2. In-Flight Snapshot Capture
```typescript
const pendingStartSnapshot = yield* snapshots.capture().pipe(Effect.forkScoped)
// ... provider streams ...
if (!publisher.hasStarted()) yield* Fiber.join(pendingStartSnapshot)
```
**Rust:** `let snapshot_rx = spawn_snapshot(); ... model_stream.await; let snapshot = snapshot_rx.await;`

### 3. Failure Classification as Pure Data
```typescript
const isInterruptedStream = (failure: AIError) => {
  if (failure.reason._tag === "InvalidProviderOutput") return failure.reason.classification === "incomplete-stream"
  if (failure.reason._tag === "Transport") return failure.reason.operation === "read"
  return false
}
```
**Rust:** `enum FailureKind { ContextOverflow, TransportRead, IncompleteStream, ... }` with `impl From<AIError>`.

### 4. Unattended Mode Overlay
```typescript
const unattended = yield* Unattended
const settled = [...all, ...unattended]
const allowed = resources.every(r => evaluate(action, r, settled).effect == "allow")
```
**Rust:** `PermissionEngine::evaluate_with_unattended(base_rules, unattended_rules)`.

### 5. Step Outcome Enum (Exhaustive)
```typescript
export type Outcome = Data.TaggedEnum<{
  Completed: { needsContinuation: boolean }
  Retry: { error, decision }
  Continue: { error, decision }
  RecoverFull: {}
  Compacted: {}
  OutputLimit: { needsContinuation: boolean }
}>
```
**Rust:** `enum StepOutcome { Completed { needs_continuation: bool }, Retry { ... }, Continue { ... }, RecoverFull, Compacted, OutputLimit { ... } }`

---

## One-Line Rust Re-Implementation Targets

| # | Target | Crate | Signature |
|---|--------|-------|-----------|
| 1 | `StepExecutor::run_step()` | `mch-core` | `async fn run_step(&mut self, ctx: StepContext) -> StepOutcome` |
| 2 | `StepExecutor::spawn_tool_concurrently()` | `mch-core` | `fn spawn_tool(&mut self, call: ToolCall) -> JoinHandle<ToolResult>` |
| 3 | `SnapshotService::capture_in_flight()` | `mch-core` | `fn capture_in_flight(&self) -> oneshot::Receiver<SnapshotId>` |
| 4 | `FailureClassifier::classify()` | `mch-core` | `fn classify(err: AIError) -> FailureAction` |
| 5 | `RetryPolicy::decide()` | `mch-core` | `async fn decide(&self, err: AIError, attempt: u32) -> RetryDecision` |
| 6 | `EventBus::publish()` | `mch-core` | `fn publish(&self, event: Event)` |
| 7 | `SkillRegistry::snapshot()` | `mch-core` | `fn snapshot(&self, perms: &Ruleset) -> (Vec<ToolDef>, ExecuteFn)` |
| 8 | `MaxStepsGuard::check()` | `mch-core` | `fn check(&self, step: u32, max: u32) -> ToolChoice` |

---

## Integration Notes for McHarness

1. **`mch-scope` already exists** — Permission engine maps directly; add `UnattendedRuleset` overlay.
2. **`mch-gate` already exists** — Completion gates map to `StepOutcome::Completed` verification.
3. **`mch-ledger` already exists** — Technique status vocabulary matches pentest-agents 12-status taxonomy.
4. **New crate needed: `mch-loop`** — The streaming step executor (this dossier's main output).
5. **New crate needed: `mch-transport`** — WebSocket transport with HTTP fallback (from model-transport.ts).
6. **New crate needed: `mch-snapshot`** — Git-tree snapshot service (from snapshot.ts).
7. **`mch-compaction` exists** — Compaction logic maps directly; precompute background fiber maps to `tokio::spawn`.

---

## Next Steps

1. ✅ **Dossier complete** — This document.
2. 🔄 **Implement `mch-loop` crate** — `loop.rs` with `StepExecutor`, `FailureClassifier`, `RetryPolicy`.
3. 🔄 **Implement `mch-transport` crate** — WebSocket transport with HTTP fallback.
4. 🔄 **Implement `mch-snapshot` crate** — Git-tree snapshot service.
5. 🔄 **Wire into `mch-core` binary** — `main.rs` uses all crates.
6. 🔄 **`cargo check` passes** — Verify compilation.
7. 🔄 **Commit + push** — `docs/ingestor/opencode.md` + `crates/mch-loop/`, `crates/mch-transport/`, `crates/mch-snapshot/`.