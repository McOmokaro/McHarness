# mercury-agent — Source-Level Dossier

Repo: `cosmicstack-labs/mercury-agent` — TypeScript ESM (Node ≥22), 3.4k★, npm-published CLI + daemon. Audited from shallow clone at v1.3.9 lineage. All paths relative to repo root.

## 1. REPO MAP

- **Single npm package** (`package.json`: name `@cosmicstack/mercury-agent`, `bin: mercury → dist/index.js`), not a workspace monorepo. Side dirs: `ui/` (React web console), `website/`, `vendor/ink/` (patched Ink TUI vendored), `patches/`, `scripts/` (build/daemon tooling).
- **Entry point**: `src/index.ts` (5,148 lines) — Commander CLI, onboarding wizard, daemon spawn (`src/cli/daemon.ts`: `startBackground`, `spawnDetachedRestart`, `readPid`, service install via `src/cli/service.js`), then `Agent` construction. `src/launcher.ts` is a secondary launcher.
- **`src/` layout**: `core/` (agent loop, scheduler, lifecycle, guards — ~40 files), `channels/` (cli, telegram, signal, discord, slack, web), `capabilities/` (tools + permissions), `soul/`, `memory/`, `sessions/`, `skills/`, `bots/` (multi-bot "fleet"), `providers/`, `cli/`, `web/`, `cloud/`, `spotify/`, `eval/`, `types/`, `utils/`.
- Tests are collocated (`*.test.ts`, vitest). `ARCHITECTURE.md` and `DECISIONS.md` (ADR files) exist at root.

## 2. AGENT LOOP & SESSION

- **Core class**: `Agent` in `src/core/agent.ts` (5,275 lines — a genuine god object). Inbound messages land in `enqueueMessage()` (line 710), which also handles bot-persona onboarding interception, then `processQueue()` (line 1667): a serialized `while (this.messageQueue.length > 0)` drain guarded by `this.processing` and `lifecycle.is('idle')`.
- **The turn**: `handleMessage(msg)` (line 2295). Order: `lifecycle.transition('thinking')` → traceId derivation → cwd scoping → slash-command short-circuit (`/budget*`, `/stream`, `handleChatCommand`) → **budget gate** (`tokenBudget.isOverBudget()` at line 2495 returns a friendly refusal with override options) → Saver-Mode auto-evaluation → session binding → history selection → `streamText` with `stopWhen: stepCountIs(opts.maxSteps)` (line 1739/3115).
- **State machine**: `src/core/lifecycle.ts` — `VALID_TRANSITIONS` table over states `unborn→birthing→onboarding→idle→thinking→responding→idle`, plus `sleeping/awakening/delegating`. `transition()` logs and rejects invalid moves. Simple, but the allowed-transition table is the honest part.
- **Sessions**: `src/sessions/repository.ts` — `SessionRepository` with JSON files (`~/.mercury/sessions/<uuid>.json`) + `index.json` holding `bindings: Record<"channelType:externalId", sessionId>`. Key API: `getOrCreateBound(channelType, externalConversationId, canonicalId)` (line 152), `bind()` (163), `appendMessage()` (187, with `externalMessageId` idempotency and monotonic `sequence`), `deletePermanently()` publishes the index *before* unlinking the session file (comment: "readers can never resolve a removed session file"). Sessions have shortIds, generated `alias` (adjective-noun wordlists, lines 17–38), `titleSource` provenance.
- **Continuation machinery**: `runInlineContinuationRound()` (line 1714) re-enters the loop with `prepareStep: forceFirstTool ? { toolChoice: 'required' }` — narration rounds are mechanically converted into action rounds. Stream truncation is classified by `classifyStreamCompletion` (`src/core/stream-completion.ts`) and continued up to `MAX_STREAM_CONTINUATIONS` (line 3217).

## 3. SOUL / PERSONA SYSTEM

- **All in `src/soul/identity.ts`** (only file in `src/soul/`). Class `Identity` manages **four markdown files** in `~/.mercury/soul/`: `soul.md`, `persona.md`, `taste.md`, `heartbeat.md` (`SoulFiles` interface, line 6). Each has a rich default template (`DEFAULT_SOUL` etc., lines 13–101) that is **written to disk on first run** (`loadOrInit`, line 178) — so users edit the live files.
- `getSystemPrompt()` (line 140) concatenates `soul + GUARDRAILS + persona`, with `{name}`/`{owner}`/`{creator_*}` template substitution. The `GUARDRAILS` block (line 103) is hardcoded — **users can edit soul/persona/taste but never the anti-drift rules**: never identify as the underlying model, never break character, and "NEVER store memories in files — the Second Brain (SQLite) is the single source of truth."
- **Identity-drift protection**: `needsMigration()` (line 197) detects files still mentioning "Cosmic Stack" branding and force-overwrites them with the new template — a migration guard that doubles as drift correction. `invalidateCache()` (174) allows hot reload.
- `getHeartbeatPrompt()` feeds the self-reflection/consolidation rules into heartbeat ticks. No MEMORY.md-style append log — persistent knowledge is deliberately banned from files and pushed into SQLite (see §7).

## 4. TOOLS & PERMISSIONS

- **Registry**: `CapabilityRegistry` (`src/capabilities/registry.ts`) — `registerAll()` (line 191) conditionally registers ~40 tools (`read_file`, `run_command`, `schedule_task`, `save_memory`, `git_*`, `delegate_task`, `dispatch_bot`…) based on manifest flags and installed integrations. `getTools()` (line 282) **filters at call time**: `install_skill` is hidden from `senderRole === 'member'` — a chat-role→code-execution escalation path closed in code. `getPlanTools()` strips all mutating tools for plan mode.
- **Permission manager**: `src/capabilities/permissions.ts` (1,157 lines). `PermissionsManifest` (filesystem scopes with `read/write/execute`, shell `blocked`/`autoApproved`/`needsApproval` globs, git read/write) persisted to `~/.mercury/permissions.yaml`, seeded from `DEFAULT_MANIFEST` (line 122).
- **Shell hardening is the crown**:
  - `splitShellSegments()` (line 211) — quote-aware segmenter that **pulls `$(...)` and backtick substitutions out as their own segments** so `echo *` can't launder `echo $(rm -rf ~)`.
  - `src/capabilities/shell/argv-lane.ts` — a second "argv lane": `tokenizePosix`, `ARGV_ALLOWLIST` (per-command arg policies), `SIDE_EFFECT_FLAGS`, `pinnedSearchDirs()`/`pinnedPath()` — binaries resolved **once at startup** against a PATH pinned to system dirs (`ensurePinnedBinaries()` called in the registry constructor, line 110). `ShellCheckResult.lane: 'argv' | 'shell'` — argv-lane approvals must execute via `execFile`, never a shell.
  - `FsAccessResult` (permissions.ts line 99) returns `canonical` realpath + `fileId {dev, ino}` so tools **fstat the opened fd and defeat symlink-swap TOCTOU**; codes: `symlink-escape`, `hardlink`, `denied`.
- **Tiers**: `src/bots/permission-tiers.ts` — `PERMISSION_TIERS: readonly|builder|operator|full` each defined as a **tool deny-list**, materialized into per-bot `permissions.yaml` via `tierPermissionsFile()`/`applyTierToManifest()`.
- **Approval flow**: `needsApproval` patterns + `approve_command` tool; per-turn scoped grants via `permissions.beginTurnGrant({scopes})` (agent.ts line 2436, ADR-016 for sub-agents); `addTempScope()` for durable per-message cwd.

## 5. TOKEN BUDGETS & RUNTIME GOVERNANCE

- **`TokenBudget`** in `src/utils/tokens.ts` (line 34): daily budget in config, persisted to `~/.mercury/token-usage.json` with a 200-entry `requestLog`, **daily rollover via `resetIfNewDay()` checked on every read** (line 200), per-agent usage map, cloud usage sync, and a `forceNext` one-shot override (`/budget override`). Restore path (line 228) **sanitizes and self-repairs** corrupted counters by recomputing from valid log entries.
- **Enforcement points**: (a) turn gate `isOverBudget()` at agent.ts:2495 — hard stop with channel-specific override UX; (b) `HISTORY_TOKEN_BUDGET = 6000` / `HISTORY_TOKEN_BUDGET_SAVER = 2400` / `HISTORY_MAX_ENTRIES = 40` / `HISTORY_MIN_ENTRIES = 2` in `src/core/context-window.ts` — `selectHistoryWindow()` budgets history *by tokens*, not message count; (c) memory extraction skipped unless `canAfford(800)` (agent.ts:4314); (d) `budgetContext()` in `system-prompt.ts` injects a "be concise" nudge above 70%.
- **Saver Mode** (`src/core/saver-mode.ts`): auto-engages at 75% usage (`saverMode.evaluateAuto`, agent.ts:2520), shrinks history budget to 2400, tracks estimated savings (`recordSavings`, lifetime counter persisted to config).
- **Runaway protection**: `ToolCallLoopDetector` (agent.ts:168) — `ABSOLUTE_MAX=75` calls, `FAILED_ABSOLUTE_MAX=20`, identical-call detection (4× same tool+params), failing-loop (6×), and **text-repetition detection via word-Jaccard ≥0.7 over last 15 step narrations**. `execution-limits.ts`: `MAX_PROVIDER_ATTEMPT_MS=10min`, `MAX_AUTOMATIC_CONTINUATIONS=6`, `MAX_AUTOMATIC_RETRIES=3`.
- **Process governance**: `memory-governor.ts` (`memoryGovernorVerdict`: ok/relief/abort) + the `memoryGuard` interval (agent.ts:2340) — stage 1 aborts generation, stage 2 **`process.exit(0)` after writing a crash flag** so the work ledger stays consistent; `StallWatchdog` (soft = status pulse, hard = abort for resume). `system-prompt.ts` documents hour-resolution timestamps to preserve provider prefix caching.
- **24/7**: `Scheduler` (`src/core/scheduler.ts`) — node-cron tasks + delayed tasks persisted to `~/.mercury/schedules.yaml`, `restorePersistedTasks()` recomputes remaining delay for `executeAt` on restart (line 193); `startHeartbeat()` ticks `heartbeatIntervalMinutes` → `Agent.heartbeat()` (agent.ts:4251): prune episodic memory (7 days), consolidate/prune Second Brain, notify at 80% budget, announce tasks due in ≤5 min. Wake reasons flow through `wakeUpPrompt()` (`execute-guard.ts`) as synthetic user turns.

## 6. MULTI-CHANNEL ACCESS

- **`Channel` interface** (`src/channels/base.ts`, line 11): `start/stop/send/sendFile/stream(AsyncIterable<string>)/typing/askToContinue/onMessage`, plus optional `sendWithActions` (tappable buttons → slash commands, ADR-023) and `askPermissionMode`. `BaseChannel` provides the `emit()` → handler plumbing. Implementations: `cli.ts`, `telegram.ts`, `signal.ts`, `discord.ts`, `slack.ts`, `web.ts`.
- **`ChannelRegistry`** (`src/channels/registry.ts`): registers CLI always, others conditionally on config; `getChannelForMessage()` and **`getNotificationChannel()`** (used by heartbeat — proactive messages need a routing policy, not a broadcast).
- **Dedup**: Telegram keys `${chatId}:${message_id}` in a `processedMessages` map cleaned every 30s (telegram.ts:155, 259). Idempotency is layered: channel dedup → `WorkLedger.accept()` key dedup (`getWorkKey()`: `request:` / `canonical:` / `channel:type:id`) → `SessionRepository.appendMessage` `externalMessageId` check.
- **Reply routing**: channels never call the agent directly; everything funnels to `enqueueMessage` and replies go back via `channels.getChannelForMessage(msg).send(...)`; internal channel type (`'internal'`) drives scheduler/memory turns with no user surface.

## 7. MEMORY & STATE

Three layers, all under `~/.mercury/`:
1. **Sessions** (`sessions/repository.ts`): full transcript per channel-conversation binding, survives restarts; feeds `selectHistoryWindow` each turn.
2. **Second Brain** (`src/memory/second-brain-db.ts`, `user-memory.ts`): SQLite (sql.js / optional better-sqlite3) of typed memories (`identity`, `preference`, `goal`, `relationship`… with `confidence/importance/durability` scores). `extractMemory()` (agent.ts:4307) runs an LLM extraction pass after each non-trivial turn (greetings regex-gated), writing 0–3 candidates. Heartbeat consolidates/prunes. Guardrail #10 forbids file-based memory.
3. **Legacy stores** (`src/memory/store.ts`): `ShortTermMemory` (JSON per conversation, capped), `LongTermMemory` (append-only `facts.jsonl` + substring search), `EpisodicMemory` (`events.jsonl`, 7-day prune with `important` tag exemption).

**Crash/exit semantics**: `src/core/work-ledger.ts` — zod-validated `work-ledger.json`; states `queued→running→paused|completed|failed|cancelled`; `recoverInterrupted()` re-queues crashed work with `workRecovered`/`continuationSummary` metadata that `handleMessage` turns into a "do not restart, continue from artifacts" prompt (agent.ts:2542); **`cancelActive()` on deliberate shutdown marks entries cancelled so restart never silently resumes killed work**; corrupt ledgers are quarantined as `.corrupt-<ts>`, never trusted.

## 8. CROWN JEWELS

1. **Two-lane shell permissioning** — `ShellCheckResult.lane: 'argv' | 'shell'` + `ARGV_ALLOWLIST` (`argv-lane.ts`). Auto-approval only when the command tokenizes to an allowlisted argv executable via `execFile` on a pinned PATH; everything else needs the human lane. *Rust: `enum ExecLane { Argv(Command), Shell(String) }` with a startup-resolved `PinnedPath` table.*
2. **Segment-splitting before pattern matching** — `splitShellSegments()` (`permissions.ts:211`) decomposes pipes/chains/`$()`/backticks and checks each segment against blocked/auto patterns. Kills allowlist laundering. *Rust: a small state-machine tokenizer over `&[u8]` returning `Vec<Segment>`.*
3. **TOCTOU-resistant file reads** — `FsAccessResult.canonical` + `fileId {dev, ino}` (`permissions.ts:99`): authorize the realpath, open it, `fstat` and compare identity. *Rust: `std::os::unix::fs::fstat` on the opened `File` vs pre-check `MetadataExt::{dev, ino}`.*
4. **Durable work ledger with honest terminal states** — `WorkLedger` (`work-ledger.ts`): persisted zod-checked entries, `markPaused` ≠ `markCompleted`, deliberate-shutdown `cancelActive()` vs crash `recoverInterrupted()`, corrupt-file quarantine. *Rust: `serde` struct + atomic temp-file-rename persist + `WorkStatus` enum with the same "cancelled never auto-resumes" invariant.*
5. **Token-budgeted history window** — `selectHistoryWindow()` with `HISTORY_TOKEN_BUDGET=6000/2400`, hard 40-entry cap, 2-entry floor (`context-window.ts:13–46`). Budget context by tokens, not turns. *Rust: a `HistoryWindow` that walks messages newest-first accumulating an estimated token count.*
6. **Soul files + immutable guardrails + template migration** — `Identity.loadOrInit()` writes editable defaults; `getSystemPrompt()` splices user-editable soul/persona around a hardcoded `GUARDRAILS` block; `needsMigration()` overwrites stale-branded files (`identity.ts:178–207`). *Rust: `SoulDir::load_or_init()` with embedded `include_str!` templates.*
7. **Cache-friendly prompt assembly** — `buildSystemPrompt()` (`system-prompt.ts:114`) is a **stable prefix**; volatile per-turn data (budget line, retrieved memory) goes in a second system block so the provider prefix cache survives; timestamps rounded to hour (`environmentSection`, line 51). *Rust: `struct TurnPrompt { stable: String, volatile: String }`.*
8. **Mechanically forced action steps** — `prepareStep` returning `{ toolChoice: 'required' }` on step 0 of continuation rounds, restricted to `FORCED_ACTION_TOOLS` (mutating only) (`agent.ts:1742`, 166). A narrating model can't burn guard rounds. *Rust: provider request middleware that rewrites `tool_choice` for round 0.*

## 9. WHAT NOT TO COPY

- **Channel zoo**: Telegram/Discord/Slack/Signal/web/Spotify, pairing-code admin approval, `getTelegramPendingRequests` etc. — product surface, not harness. Take only the `Channel` trait idea.
- **Mercury Cloud**: JWT usage sync (`syncToCloud`), hosted provider, web server auth (`web/auth.ts`, bcrypt), `ui/` React console.
- **Bot fleet**: `bots/` (BotManager, `refinePersona`, `proposeCrew`, bundles, `dispatch_bot`), sub-agent supervisor + kanban board — interesting but orthogonal to a terminal harness.
- **Monolith `agent.ts`**: 5,275 lines, 60+ imports; slash commands, Spotify TUI, onboarding, and memory UI all inline. Extract-by-default.
- **`Identity` anti-drift rules** (never reveal the model) are a consumer-product persona choice; a terminal harness should not lie about its model.
- **Bun/SEA packaging, Ink vendoring (`vendor/ink/`, `scripts/vendor-ink.cjs`), dev-channel version stamping** — distribution plumbing irrelevant to a Rust binary.
- **God-tier global singletons** (`logger`, `setAskUserHandler` module-global in `capabilities/interaction/ask-user.js`) — Rust ownership makes this a smell; pass contexts explicitly.
