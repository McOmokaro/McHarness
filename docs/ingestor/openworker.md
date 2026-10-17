# OpenWorker (andrewyng/openworker) — Source-Level Dossier

> Lineage: Security Team / task-oriented worker agent — scheduled autonomous execution. Audit of local clone, verified against source.

Python agent-worker platform ("coworker" is the package name everywhere). Launch modes: `openworker tui` (Textual TUI), `openworker run` (headless), `openworker-server` / GUI sidecar (uvicorn FastAPI), plus a `machine`/`join` remote-control CLI. All paths relative to repo root.

## 1. REPO MAP

- `coworker/` — the entire core: `engine.py` (TurnEngine), `cli.py`, `tools/`, `automation/`, `providers/`, `server/`, `tui/`, `headless/`, `memory/`, `teams/`, `permissions.py`, `selfwake.py`, `compaction.py`, `sandbox/`, `mcp/`, `reviewer.py`.
- Entry points (`pyproject.toml [project.scripts]`, `coworker/cli.py:main`): dispatches `run` → `headless/runner.py:main`, `machine`/`join` → `remote/joiner.py:cli`, everything else → `tui/app.py:CoworkerApp`. The always-on deployment is `coworker/server/run.py` (uvicorn) hosting `server/app.py:create_app` (OpenAI-compatible endpoint + WS session API + REST).
- `surfaces/gui/` — desktop GUI (Tauri-style sidecar over the server). `stt/` — speech-to-text. `tests/`, `docs/`, `packaging/`, `scripts/`, `reports/` — support.
- Deployment model: a **single-user, local-first daemon** — the server owns sessions, automations, and the scheduler; GUI/TUI/remote are clients over WS/REST.

## 2. CORE AGENT LOOP (`coworker/engine.py:TurnEngine`)

The loop is `TurnEngine._loop()` (engine.py:638), an async generator yielding typed `Event`s (`events.py`). One user turn = up to `max_iterations` (default 12) model↔tool rounds:

1. **Interrupt repair** — `run()` first calls `_repair_dangling_tool_calls()` (line 576): any orphaned `tool_use` in history (restart mid-approval) gets an honest stub tool-error result, because providers hard-reject dangling calls.
2. **Compaction checkpoint** — `_compaction_due()` (cheap, side-effect free) then `_compact_now()`; on provider context-overflow 400 it compacts mid-loop with a progress-guarded retry.
3. **Streaming** — `_astream()` (line 1046) bridges the blocking provider stream to asyncio via a thread+queue, racing chunks against the cancel event so a wedged stream can't hold the turn.
4. **Tool handling** — `_handle_tool_calls()` (line 1095): authorize ALL calls first (sequential, interactive), then execute. **Risk-tiered concurrency**: `_parallel_safe()` runs `risk_level == "low"` tools concurrently via `asyncio.gather`; writes/shell run strictly ordered in call order.
5. **Termination classes** — `completed`, `max_iterations_exceeded`, `interrupted`, `truncated`, `sleeping` (self-wake), plus OPE-171 continuation: a length-truncated reply with no tool call gets a `TRUNCATION_NUDGE` (max 2), else ends as `truncated`, never fake-"completed".
6. **Steering** — `queue_steering()` lets the UI inject mid-turn messages; `_inject_steering()` runs at tool-turn boundaries.
7. **Durable resume** — `resume()` (line 554) reconstructs unanswered trailing tool calls from the persisted thread and re-executes only those (prompt callbacks find already-resolved inbox items, so nothing double-fires).

Cancellation is a single `asyncio.Event` + `_interruptible()` racing every await; `interrupt_hooks` kill running shell processes.

## 3. TASK MODEL & SCHEDULING (`coworker/automation/`)

- `models.py:ScheduledTask` — persistent entity: `title`, `instructions` (the prompt), `Schedule` (`cron` | `once`, with timezone), `workspace`, `model`, `max_runs`, counters (`run_count`, `last_status`), and **task-scoped standing approvals** (`always_allowed_tools` as `"tool target"` entries, `standing_rules()`/`name_allowed_tools()`) so an automation pre-grants narrow, target-bound permissions.
- `store.py:TaskStore` — SQLite, JSON blobs with indexed `next_run`/`enabled` columns; `compute_next_run` uses croniter (TZ-aware: naive base → local wall-clock, explicit comment on the DST trap). `save()` recomputes `next_run`.
- `scheduler.py:Scheduler` — 30s tick asyncio loop. Two policies stated in the docstring: **run-once-catch-up** (missed tasks fire once on startup) and **skip-on-overlap**. The critical trick is in `_tick()` (line 76): the overlap guard `_claim()` is taken **in the tick, before the spawn**, because the `due()` snapshot goes stale — a guard checked inside the spawned task races a fast-finishing previous run and the task would run twice. Runs spawn, never await (a parked approval must not stall the loop). `runner` is injected (`Callable[[ScheduledTask, str], TaskRun]`), so the scheduler is engine-agnostic.
- **Self-wake** (`selfwake.py`) — the starkest autonomy pattern: the agent itself calls `sleep_for` / `sleep_until` / `wake_on(job_id)` / `wake_on_event(event_key)` tools (marked `__coworker_yields_turn__`), the turn ends `sleeping`, and `WakeStore` (JSON-file persisted, atomic tmp-replace, with in-`_load` migration collapsing duplicate pending timers) hands due wakes to the scheduler's `extra_tick`. Result: event-driven suspend/resume with ~zero idle cost. `WakeStore.acknowledge()` only fires after a durable message receipt.

## 4. TOOL SYSTEM

- `tools/registry.py:ToolRegistry` — deliberately thin: `ToolSpec{name, schema, func, metadata}`; schema generation reused from aisuite's docstring/type-hint extractor (`_schema_for`), with `__coworker_schema__` escape hatch. **No permission logic in the registry** — enforcement lives in `PermissionEngine`, applied by the engine. Risk metadata (`risk_level`, `requires_approval`) drives both approval and the parallelism decision.
- Built-in tools: `shell.py`, `files.py`, `git.py`, `search.py`, `network.py`, `plan.py` (`propose_plan`), `todo.py`, `subagent.py`, `ask.py` (`ask_user`), `toolreq.py` (`request_tool`), `directories.py` (`request_directory`), `connreq.py`. Interactive "gates" (`propose_plan`, `ask_user`, `request_directory`, `propose_team`, `propose_work_items`) bypass the normal permission path — the human's out-of-band decision IS the consent (engine `_handle_tool_calls` dispatch, lines 1128–1156).
- MCP supported via `coworker/mcp/`; destination stamped at registration as `func.__coworker_mcp_destination__` so approval cards show "leaves this computer → host".
- No browser/computer-use tool found; web access is `network.py`/`search.py` behind a site wall.

## 5. MODEL & MEMORY LAYER

- `providers/` — one `ProviderClient` abstraction over Anthropic, OpenAI (+Responses), Gemini, Bedrock, Vertex, Ollama/local server, OpenRouter, Codex; `matrix.py` holds context windows/labels; `router.py` routes. Canonical history is OpenAI shape, converted per call — which is what makes `switch_model()` mid-session nearly free (engine.py:451), with vision-capability degradation notices.
- **Compaction** (`compaction.py` + engine §879–1043): token-signal-triggered (reported usage preferred over estimates), thresholds per-model with a one-time warning when compacting at a guessed 128k for a big-window model (OPE-170); budgets scale with the trigger so a lowered threshold can't be undone; summary + verbatim transcript spill file; failure policy differs attended (Retry/Trim prompt) vs unattended (auto-trim, never park a run).
- **Ephemeral context block**: `<system-context>` tag (`EPHEMERAL_CONTEXT_OPEN`, engine.py:55) — volatile per-turn context (live clock, directory list) rides the last user message so providers keep their cache breakpoint on the last *stable* block.
- Memory: `memory/` with `SQLiteMemoryStore` (`coworker.db`) + `MemorySettingsStore` ("off means stop learning, saved facts stay usable"); `session_facts.py` freezes the world at session start; `provenance.py:SessionFiles` tracks files the agent wrote/downloaded this session.
- **Tool-result bounding** (`toolresult.py`): results truncated head+marker+tail at 10k bytes, full text spilled to disk — history stays small, detail recoverable by a read.

## 6. SURFACES

- `surfaces/gui/` — desktop app driving the FastAPI server (`server/app.py`: OpenAI-compatible endpoint + WS sessions + REST).
- `tui/app.py:CoworkerApp` — Textual TUI; default surface for `openworker tui`, launching a named "skill".
- `headless/runner.py` — `openworker run` for scripted/CI use; `headless/trajectory.py` + `pricing.py` export run trajectories with cost.
- `remote/joiner.py` — pair a remote machine to a server (`join`, `machine` commands). All surfaces consume the same event stream; none embed agent logic.

## 7. CROWN JEWELS

1. **Pre-spawn overlap claim** — `automation/scheduler.py:Scheduler._tick` + `_claim`: claim the skip-on-overlap guard in the tick against a stale `due()` snapshot, before spawning. One-line Rust target: a `HashSet<TaskId>` claimed in the tick loop before `tokio::spawn`.
2. **Injected runner scheduler** — `Scheduler(store, runner, tick_seconds, extra_tick)`: scheduling is fully decoupled from execution and gains a free second consumer (wake resume). Rust target: generic `Scheduler<R: Fn(Task, Trigger) -> Future<TaskRun>>` on a tokio interval.
3. **Self-wake suspend/resume** — `selfwake.py:WakeStore` + `selfwake_tools` (`sleep_for/sleep_until/wake_on/wake_on_event`, `__coworker_yields_turn__`): the model schedules its own resumption; turn ends `sleeping`. Rust target: a `WakeStore` (JSONL/sled) consumed by the scheduler tick, engine returning a `TurnEnd::Sleeping` status.
4. **Dangling tool-call repair + durable resume** — `engine.py:_repair_dangling_tool_calls` and `resume()`/`_unanswered_trailing_tool_calls`: close orphaned calls with honest stubs; on restart, re-execute only the unanswered trailing calls (idempotent callbacks prevent double-fire). Rust target: transcript-repair pass on session load + `Resume` re-entry executing pending `ToolCall`s.
5. **Risk-tiered tool parallelism** — `engine.py:_parallel_safe` + `_handle_tool_calls`: metadata-declared low-risk tools fan out via `gather`, writes stay ordered. Rust target: `futures::join_all` for `risk_level == Low`, sequential otherwise.
6. **Reviewer with streak-trip breaker + shadow mode** — `engine.py:_preconsult_reviewer/_authorize`, `_REVIEWER_TRIP = 5`, `reviewer_shadow`: concurrent LLM verdicts on all escalations before the sequential authorize; deny streak trips to hand-approval (with persisted notice); shadow verdicts audited without touching decisions. Rust target: a `Reviewer` trait consulted before card emission, with a consecutive-denial circuit breaker.
7. **Provenance floor: fetch-then-execute** — `engine.py:_downloaded_target/_provenance` + `provenance.py:SessionFiles`: a file the agent downloaded this session can never be executed without a human, over any allowlist — "the floor only ever tightens an allow". Rust target: per-session file-provenance set consulted in the permission evaluator.
8. **Attendance model (never a hang)** — `engine.py:_nobody_answers_cards/_auto_answering` + `unattended.py`: when no person can answer, escalation resolves by fixed rule to a recorded "no" instead of blocking. Rust target: an `Attendance` enum (`Attended|Inbox|Auto`) consulted in the approval path with auto-deny + audit row.

## 8. WHAT NOT TO COPY

- **Product surfaces** — `surfaces/gui/` desktop app, Tauri/sidecar packaging (`packaging/`), `ui-mocks/`.
- **STT** (`stt/`) — speech input is a UI concern orthogonal to the harness.
- **Remote pairing/`machine` CLI** (`remote/`) — multi-machine fleet join is deployment-specific.
- **Provider zoo details** — 10+ provider adapters, pricing tables, recommended-model JSONs; Rust harness should define one trait and port adapters lazily.
- **Teams/boards** (`teams/`, `proposals.py`) — multi-agent lead/worker decomposition with human staffing gates is a product layer; its *approval delegation* (`decide_worker_call`, `_delegated_context`) is the only piece worth mining later.
- **Connector/inbox routing** (`connectors/`, `inbox*.py`, `mentions.py`) — messaging-platform integration specifics (Telegram-style targets) are surface glue, not harness pattern.
