# n8n — Source-Level Dossier

Repo: `n8n-io/n8n` — TypeScript, ~208k★, branch `master`, "Fair-code workflow automation platform with native AI capabilities, 400+ integrations". Audited from shallow clone.

## 1. REPO MAP

pnpm + turbo monorepo. Root: `package.json`, `turbo.json`, `pnpm-workspace.yaml`. Key locations:

| Area | Path | Role |
|---|---|---|
| Shared workflow model | `packages/workflow/src/` (`workflow.ts`, `graph/`, `expressions/`) | Node/connection model, expression evaluator — the vocabulary both engine and editor share |
| Execution engine | `packages/core/src/execution-engine/` (**not** `packages/core/workflow-engine/` — that path does not exist in this tree; `packages/core/` has only `bin/ src/ test/ nodes-testing/`) | `WorkflowExecute`, trigger/poller machinery |
| CLI/orchestration | `packages/cli/src/` | `active-workflow-manager.ts`, `webhooks/`, scheduling, DB repos |
| Nodes | `packages/nodes-base/`, `packages/@n8n/nodes-langchain/` | ~400 integrations; AI/agent nodes |
| AI packages | `packages/@n8n/`: `ai-workflow-builder.ee/`, `nodes-langchain/`, `ai-node-sdk/`, `engine/`, `task-runner/`, `task-runner-python/`, `scheduler/`, `permissions/`, `db/` | LangChain integration, sandboxed task runners, DB entities |
| Entry points | `packages/cli/src/index.ts` (server), `packages/cli/src/commands/` (CLI verbs) | |

## 2. WORKFLOW GRAPH MODEL

**Core model — `packages/workflow/src/workflow.ts`:** `export class Workflow` (line 58) holds `nodes`, `connectionsBySourceNode: IConnections` (line 65) and the derived index `connectionsByDestinationNode` (line 67) built via `mapConnectionsByDestination` (line 146). Connections are keyed `sourceNode → connectionType ("main"/"ai_tool"/…) → outputIndex → IConnection[]` where `IConnection = { node, type, index }`.

**Graph utilities — `packages/workflow/src/graph/graph-utils.ts`:** `buildAdjacencyList` (line 170) → `IConnectionAdjacencyList = Map<string, Set<IConnection>>` (line 36); `getRootNodes` (103), `getLeafNodes` (123), `hasPath` (145), `parseExtractableSubgraphSelection` (239). Validation lives in `packages/workflow/src/workflow-validation.ts` / `node-validation.ts` / `workflow-structure-validation.ts`.

**Node metadata:** `INodeTypeDescription` (in `packages/workflow/src/interfaces.ts`, which also defines `retryOnFail`/`maxTries` at lines 1760–1761, `alwaysOutputData` 1763, `continueOnFail` 1766, `settings.errorWorkflow` 4109) declares `inputs/outputs/credentials/requiredInputs`. Node behavior schema is zod-validated in `packages/workflow/src/schemas.ts` (lines 481–487). `packages/workflow/src/versioned-node-type.ts` supports `typeVersion`-dispatched node types.

**Pinning:** `IPinData` = `Map nodeName → output items`, injected into `WorkflowExecute` (workflow-execute.ts line 101) and consulted per run via `getPinnedOutput` (line 2377): a pinned node returns canned output instead of executing.

## 3. EXECUTION ENGINE

`packages/core/src/execution-engine/workflow-execute.ts` (3,227 lines), `export class WorkflowExecute` (line 119).

- **Scheduling:** not topological-sort; it is a **stack simulation**: `runExecutionData.executionData.nodeExecutionStack` plus a deferred map `waitingExecution[nodeName][runIndex] = { main: [...inputs] }`. The main loop pops a node, checks `ensureInputData` (line 2883; also uses `workflow.getHighestNode` to skip disabled ancestors), executes, then walks `connectionsBySourceNode` to push successors (line 2537 ff.). Under `settings.executionOrder === 'v1'` successors are **sorted by canvas position — top-most then left-most** (line 2601–2616) — the visual layout *is* the deterministic schedule. When the stack empties, `waitingExecution` drains node-by-node, honoring `nodeType.description.requiredInputs` so multi-input nodes (Merge) wait for all inputs (lines 2642–2799).
- **Branching:** empty output array ⇒ branch dies (`continue executionLoop`, line 2458–2462); two-output nodes route items by outputIndex.
- **Retries:** `getRetryParams(executionData)` (line 2358) reads `retryOnFail`/`maxTries`; the retry loop (2360–2469) sleeps `waitBetweenTries` and re-runs `runNode` — note the inner retry-while (2402–2417) keyed off a **json error sentinel**: `checkFailure` treats `output.json.error` as failure even when the node "succeeded" (line 2356).
- **continueOnFail / error workflows:** failure handling funnels into `handleNodeExecutionError` (line 2482) which returns `{ continueExecution }` — that implements `continueOnFail` and `settings.errorWorkflow` dispatch.
- **Sub-workflows:** the `executeWorkflow` node calls back into the engine with a child `WorkflowExecute`; `runNodeFilter` (line 186–201) implements **partial execution of single nodes** (`destinationNode`), and when `runExecutionData.waitTill` is set the engine re-pushes the node (`pushExecutionStack`, line 2517) and persists state between hook `nodeExecuteAfter`.
- **Waiting/timers:** `waitTill` on `IRun` suspends the run; the Wait node (in `packages/nodes-base/nodes/Wait/`, versioned subfolders) resumes it via webhook or timer.
- **Data model:** `IExecuteData = { node, data: { main: INodeExecutionData[][] }, source }`; results accumulate in `runExecutionData.resultData.runData[nodeName][runIndex]` (`ITaskData`, `upsertTaskData` line 2507). Items are **paired** to their producing item via `assignPairedItems` (line 2450).

## 4. AI AGENT NODE

All under `packages/@n8n/nodes-langchain/nodes/agents/Agent/`:

- `V3/AgentV3.node.ts` — `export class AgentV3 implements INodeType` (line 25); `execute()` (line 144) delegates to `toolsAgentExecute` from `agents/ToolsAgent/V3/execute.ts`.
- **The loop is engine-mediated, not in-process:** `toolsAgentExecute` (line 59) builds context (`buildExecutionContext` — model, fallbackModel, memory, batching), processes items in batches via `executeBatch`, and if the LLM emitted tool calls it **returns an `EngineRequest`** (line 132) instead of calling tools itself. `WorkflowExecute.handleEngineRequest` (workflow-execute.ts line 2421) schedules the referenced tool *nodes* and resumes the agent with an `EngineResponse` — see the `nodeWasResumed` guard at line 2352 that prevents double `nodeExecuteBefore` hooks on resume. Sub-agents that cannot reach the engine use `resolveSubAgentRequest` (line 138) to run tools inline.
- **Tools are nodes:** `ToolExecutor.node.ts` (`export class ToolExecutor`, line 26; inputs `[Main, AiTool]`) plus per-node tool wrappers under `nodes/tools/` and MCP clients under `nodes/mcp/` (`McpClientTool`, `McpTrigger` with `execution/ExecutionCoordinator.ts`, `QueuedExecutionStrategy.ts`). Connection types `NodeConnectionTypes.AiTool/AiMemory/AiLanguageModel` wire memory/tools/model as **typed canvas edges**, gathered via `getInputs` (`nodes/agents/utils.ts`) / `getInputConnectionData`.
- **Human-in-the-loop:** no dedicated approval primitive found in the agent path; the closest mechanisms are the `EngineRequest`/`EngineResponse` pause-resume protocol and `waitTill` webhooks.
- Tracing metadata keys `ai.agent.tool_calls.requested/completed`, `ai.agent.iteration.count` etc. (lines 20–32) are emitted in `finally` — a good observability pattern.

## 5. TRIGGERS & SCHEDULING

- **`packages/cli/src/active-workflow-manager.ts` — `export class ActiveWorkflowManager` (line 75):** on activation it distinguishes **webhook, poll, and schedule triggers** (comment at line 526–533), registers in-memory via `this.activeWorkflowTriggers.addTriggers(...)` (line 1165, backed by `packages/core/src/execution-engine/active-workflow-triggers.ts`), and errors if a workflow "has no node to start the workflow" (line 634).
- **Cron:** `packages/core/src/execution-engine/scheduled-task-manager.ts` — `export class ScheduledTaskManager` (line 28) wraps `CronJob` grouped in `cronsByGroupType = Map<groupType, Map<groupId, Set<CronJob>>>` (line 29); `register(ctx, onTick)` (line 67) skips registration on follower instances in multi-main mode — leadership-gated cron.
- **Polling:** `PollTriggerJobRegistrar` (`cli/src/scheduling/poll-trigger-node/`) + engine-side `poll-trigger-executor.ts`, `poll-job-manager.ts`; dedup of external events via `data-deduplication-service.ts`.
- **Webhooks:** `cli/src/webhooks/` — `test-webhooks.ts` `export class TestWebhooks` (line 64) implements preview-webhook lifecycle (`IWebhookManager`); production registration persists webhook routes on activation. `trigger-identity.ts` and `trigger-credential-gate.ts` (workflow pkg) handle registration identity and credential-gated triggers.

## 6. CREDENTIALS & SECURITY

- **Encryption:** `packages/core/src/encryption/` — `cipher.ts`, `aes-256-cbc.ts`, `aes-256-gcm.ts`, `encryption-key-proxy.ts`; credentials are encrypted at rest with the instance key, decrypted into node contexts only.
- **Permission scoping:** `packages/@n8n/permissions/src/` — `types.ee.ts` (`export type Scope` line 41, `ScopeLevels` 43, `RoleNamespace` 55) and `schemas.ee.ts` (`export type Role = z.infer<typeof roleSchema>` line 91); roles are zod schemas over scope sets, projects own workflows/credentials (project/role DB entities under `packages/@n8n/db/`).
- **Redaction:** `packages/workflow/src/auth-redaction.ts` and `redaction-channels.ts` exist for auth/secret scrubbing; expression evaluation is sandboxed via `expression-sandboxing.ts` and `expression-evaluator-proxy.ts`, plus `safe-regex.ts` (ReDoS guard).

## 7. PERSISTENCE & STATE

- **Execution history:** run data persisted as `IRun` per execution via execution repositories (`packages/@n8n/db/`); `resultData.runData` is the full per-node transcript.
- **Pruning:** soft-delete columns plus `ExecutionRepository.hardDeleteSoftDeletedExecutions` (referenced in migration `packages/@n8n/db/src/migrations/common/1723796243146-RefactorExecutionIndices.ts`, line 27); sqlite migration `MigrateIntegerKeysToString.ts` shows the `pruneExecutionsData` helper (line 202).
- **Active-workflow state:** `active-workflow-manager.ts` + `workflow-active-triggers-state.ts` (core) track per-workflow trigger registrations; **static data** (`workflow.staticData`, `__dataChanged` flag at workflow-execute.ts line 2834) survives restarts for poll-cursor/dedup state, persisted via the `workflowExecuteAfter` hook (line 2841).
- **Interruptible runs:** `waitTill` runs are serialized (stack + waitingExecution) to DB and resumed on webhook/timer — restart-safe long waits.

## 8. CROWN JEWELS

1. **Execution-stack dataflow scheduler** — `WorkflowExecute` loop, `nodeExecutionStack` + `waitingExecution` (workflow-execute.ts 2330–2800). Clever: no topo-sort re-planning; data *arrival* drives scheduling, naturally handling merges, branches, and dead branches. *Rust:* a `HashMap<NodeId, Vec<Option<Vec<Item>>>>` input-accumulator + worklist loop.
2. **Canvas-position deterministic ordering** — nodesToAdd sort by `[x, y]` (lines 2601–2616). Clever: user-drawn layout doubles as a total order. *Rust:* `(row, col)` field on node defs; sort successors by it.
3. **Pinned outputs** — `getPinnedOutput` (line 2377) + `IPinData`. Clever: cache-as-control-flow for iterative workflow authoring. *Rust:* per-node cached `Vec<Item>` bypassing execution — ideal for harness step replays.
4. **json-error sentinel + retryOnFail** — `checkFailure` (line 2356) + retry loops (2360–2417) with `maxTries`/`waitBetweenTries` from node settings. Clever: failure is *data* on a technically-successful run, so retries stay uniform. *Rust:* `Result<Output, String>` carried in output payload; retry driver reads it.
5. **EngineRequest / EngineResponse tool-call protocol** — Tools Agent V3 (`ToolsAgent/V3/execute.ts` line 59) returns `EngineRequest`; `handleEngineRequest` (workflow-execute.ts 2420–2431) runs tool nodes and resumes with `EngineResponse`; `nodeWasResumed` guard (2352). Clever: agent loop becomes a *suspendable coroutine of the workflow engine* — tool execution, permissions, and tracing all reuse node machinery. *Rust:* model tool calls as a `Step::Yield(ToolCallBatch)` returned up to the harness scheduler.
6. **requiredInputs / waitingExecution barrier** — lines 2668–2742. Clever: declarative "execute only when N inputs have data" solves multi-parent joins without phasing. *Rust:* `required_inputs: Option<BitSet>` per node checked against the accumulator.
7. **Leadership-gated cron** — `ScheduledTaskManager.register` skipping on followers (scheduled-task-manager.ts line 71). Clever: HA-safe scheduling with one `bool`. *Rust:* `if !is_leader { return false }` guard inside timer registration.
8. **Hook-driven lifecycle** — `nodeExecuteBefore/After`, `workflowExecuteAfter` via `hooks.runHook` (lines 2353, 2636, 2841). Clever: persistence, UI, and telemetry are all subscribers to a single event spine. *Rust:* a `trait ExecutionHook` with before/after-node methods driving journaling and tracing.

## 9. WHAT NOT TO COPY

- **The whole editor UI** (`packages/frontend/`) and canvas/selection code — irrelevant to a terminal harness.
- **`settings.executionOrder === 'v1'` legacy branch** and duplicated legacy retry/`isLegacyExecutionOrder` paths (workflow-execute.ts 2901–2913) — copy only the v1 semantics.
- **Multi-instance/multi-main leader election, pub/sub, and scaling** (`@n8n/broadcast`/scaling layers) — a single-process harness needs none.
- **TypeORM + 400+ migration files** (`packages/@n8n/db/src/migrations/`) — use a simple append-only journal instead.
- **Expression sandboxing at JSVM level** (`expression-runtime/`, `native-methods/`) — Rust has no eval; use a typed templating layer.
- **Product-surface nodes** (HTTP/SaaS integrations in `nodes-base/`, OAuth flows, `chat-hub`, `computer-use`, `mcp-browser*`) — keep only the *tool-node shape*, not the inventory.
- **EE-licensed gating** (`*.ee/` packages, `LICENSE_EE.md`) — license surface, not engineering.
- **Eval/mock fixtures in engine code** (`eval-mock-fixtures.ts`, `eval-mock-helpers.ts` inside `execution-engine/`) — keep evaluation out of the core executor.
