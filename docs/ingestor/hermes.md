# Ingestor Dossier: Hermes (Nous Research)

Source: `NousResearch/hermes-agent` — full shallow clone, audited file-by-file. Python. Every claim below is verified against actual code paths; nothing is inferred from docs.

## Repo Map

Entry point `hermes` → `hermes_cli/main.py`. The core is **`agent/`** (~400 modules). Session persistence is a flat family of `hermes_state*.py` mixin modules composed into one `SessionDB` class. Supporting trees: `tools/` (tool implementations), `skills/` + `optional-skills/` (SKILL.md dirs), `providers/`, `gateway/` (multi-channel serving), `cron/`, `evals/` (including a compaction evaluation runner), `native/fts5_cjk/` (custom C tokenizer).

Key verified modules:

- Loop: `agent/conversation_loop.py`; turn phases split across 25+ `agent/turn_*.py` files
- Prompt: `agent/prompt_builder.py`, `agent/system_prompt.py`, `agent/prompt_caching.py`, `agent/prompt_cache_boundary.py`
- Compression: `agent/context_compressor.py` (5,824 lines), `agent/micro_compaction.py`, `agent/conversation_compression*.py`, `agent/context_engine.py`, `agent/context_pin.py`
- Memory: `agent/memory_provider.py`, `agent/memory_manager.py`, `tools/memory_tool.py`; search in `hermes_state_search.py`, `hermes_state_fts.py`, `hermes_state_schema.py`
- Skills: `agent/skill_commands.py`, `agent/skill_utils.py`, `agent/skill_preprocessing.py`, `agent/learning_graph.py`, `agent/learning_mutations.py`, `agent/curator.py`, `tools/skill_manager_tool.py`, `tools/skill_usage.py`
- Dispatch: `agent/tool_executor.py`, `agent/tool_dispatch_helpers.py`, `agent/tool_guardrails.py`, `toolsets.py`

## The Agent Loop

`agent/conversation_loop.py::run_conversation(agent, ...)` drives one user turn, decomposed into phase modules imported and bound at module top — deliberately, so a source-tree swap cannot load a skewed phase mid-turn:

1. `turn_iteration_prep.py::begin_iteration / prepare_iteration / announce_api_call`
2. `turn_preflight_gate.py::run_preflight_gate` — pre-API compression guard. Notable: `_midturn_request_pressure_tokens` prefers the native Responses-API wire estimate over the generic estimate, because generic estimates overstate the wire "by orders of magnitude on a compacted session" and caused spurious compressions (issue #96995).
3. `turn_request_assembly.py::assemble_api_request` / `turn_api_request.py::build_api_request`
4. `turn_api_call.py::perform_api_call` (with `handle_api_interrupt`, `nous_rate_limit_guard`)
5. `turn_response_check.py::check_api_response` → `turn_response_intake.py::normalize_model_response`
6. `turn_tool_round.py::run_tool_round` — loops until no tool calls
7. `turn_final_response.py::finish_text_response` → `turn_finalizer.py::finalize_turn`

Error handling: `turn_loop_errors.py::handle_outer_loop_error` with `_MAX_OUTER_LOOP_ERRORS = 8`, and a traceback classifier (`_LOCAL_PROCESSING_MODULES` vs `_API_CALL_MODULES`) that refuses to retry stacks classified as "deterministic local bug" — retrying local bugs is waste, retrying API hiccups is recovery.

Notable mechanics:

- `_maybe_inject_run_budget_wrapup`: at 80% of `--run-budget`, latches a one-time "stop discovery, produce deliverable" notice, appended **only to the newest non-persisted `tool` message** — placement chosen for prompt-cache safety.
- `_continuation_overlap_length` uses KMP string matching to deduplicate seams ≥32 chars where an interrupted stream was resumed, so the transcript does not carry duplicated text at resume boundaries.
- Mixture-of-agents (MoA) is a client facade detected via `MoAChatCompletions.prepare` (`_moa_client_consumes_prepared_request`) rather than a separate loop path.

## Memory

**Storage**: SQLite via `hermes_state.py` + mixin family. `hermes_state_schema.py` holds `SCHEMA_SQL` and column reconciliation. Sessions carry a **content-addressed system prompt** (`_store_system_prompt`, `system_prompt_hash`; legacy inline prompts deduped by `_dedupe_legacy_system_prompts`, schema v25). Durability infrastructure: `hermes_state_wal.py`, `hermes_state_readpool.py`, `hermes_state_lockguard.py`, `hermes_state_repair.py`, `hermes_state_rewind.py`.

**FTS**: three FTS5 indexes over external content — `messages_fts` (base), `messages_fts_trigram`, `messages_fts_cjk` — all indexing views (`messages_fts_src`, `messages_fts_cjk_src`) that exclude `role='tool'` rows, over `content, tool_name, tool_calls`. The CJK index uses a custom loadable tokenizer `cjk_unicode61` (`native/fts5_cjk/fts5_cjk.c`, ~250 lines of C) that re-emits CJK runs as overlapping bigrams, because trigram needs ≥3 characters and 1–2 char CJK queries fell through to 3–6 second LIKE scans.

Sync triggers are **gated on `state_meta` markers** (`fts_cjk_rebuild_high_water` / `fts_rebuild_progress`): a row is indexed only if its id is above the high-water mark or below rebuild progress, so a background backfill coexists with live writes. Failure policy is fail-open: if the tokenizer cannot load, `_ensure_fts_cjk_schema` **drops the triggers and writes a stale breadcrumb** (`FTS_CJK_STALE_KEY`) — rationale in comments: an external-content "delete" for a rowid the index never held is the canonical FTS5 corruption hazard, so writes must never fail at trigger time. `_drop_orphan_fts_shadow_tables` repairs `.recover`-emitted orphan shadow tables.

**Search/ranking** (`hermes_state_search.py`): primary route is FTS5 BM25 — `sort=None` → `ORDER BY rank`; newest/oldest → timestamp-then-rank (`_FTS_ORDER_BY`). A capability-gated route chain: cjk (exact ranked match) → trigram → LIKE; the LIKE path over-fetches then ranks in Python with a `score(row)` function, stable-sorted by `(score, index)`. Query strings are sanitized (quoting dotted/hyphenated terms in one pass).

**Providers**: `agent/memory_provider.py::MemoryProvider` ABC — lifecycle `initialize(session_id, hermes_home)` → per-turn `system_prompt_block()` (static) + `prefetch(query)` (background `queue_prefetch`, consumed next turn) + `sync_turn(...)` (non-blocking) → `get_tool_schemas()`/`handle_tool_call()` → `shutdown()`. Hooks: `on_pre_compress` (feeds extraction into the summary prompt; v2 is a fail-closed checkpoint API), `on_memory_write` (mirrors built-in writes with `previous_content` provenance), `on_session_switch`, `identity_signature` (gateway cache-busting). `is_trivial_prompt` regex skips recall for acks/greetings. Threading: `ctx_bound`/`spawn_context_thread` copy the spawner's contextvars so profile-scoped `HERMES_HOME` and secret scopes survive onto worker threads.

Built-in store: `tools/memory_tool.py::MemoryStore` — §-delimited `MEMORY.md`/`USER.md`, char budget, `_mutate` cross-process lock with `.bak` snapshot and round-trip drift refusal.

## Skills

Skills are directories with `SKILL.md` (frontmatter parsed by `agent/skill_utils.py::parse_frontmatter`; description/conditions extracted, platform/app/environment matching via `skill_matches_*`).

**Discovery**: `agent/skill_commands.py::get_skill_commands` scans tiered roots (`get_skill_search_roots`: local/external/project tiers), memoized in a **multi-slot cache keyed by (platform, hermes_home, project root)** with an on-disk signature so a skill installed from another terminal triggers a rescan. `slugify_skill_name` keeps Unicode so `name: 小说拆条` registers as `/小说拆条`.

**Invocation**: `/skill` expands into a model-facing scaffold — header `_SKILL_INVOCATION_PREFIX = '[IMPORTANT: The user has invoked the "'` + name + `_SINGLE_SKILL_MARKER`, full body, footer telling the model to run scripts with the terminal tool using absolute paths — with the user instruction appended by `append_user_instruction`, which returns the **stable prefix** registered with `agent.prompt_cache_boundary` so the cache planner can split exactly at the scaffold/instruction boundary. Single construction site guarantees the byte-prefix invariant. Bundles and gateway auto-loads are sibling scaffolds; `extract_user_instruction_from_skill_message`, `describe_skill_invocation`, `skills_loaded_by_scaffold` reverse them for previews/titles.

**Learning loop**: prompt guidance (`prompt_builder.py::SKILLS_GUIDANCE`, `build_memory_guidance`) teaches the model a placement rule: procedures/pitfalls/user-corrections belong in the skill that was used (via `skill_manage`); memory is the narrow exception for every-session facts, written as declarative facts not imperatives. Write-back tooling: `tools/skill_manager_tool.py` (`_edit_skill`, `_find_skill`), `tools/skill_usage.py` (`archive_skill`, pinning), `agent/curator.py` (autonomous maintenance that skips pinned skills — even the background review fork may not write pinned skills, issue #25839).

`agent/learning_graph.py` mints "journey" node ids — skills → name; memories → `memory:<source>:<index>:<fingerprint>` where fingerprint = `memory_fingerprint(text)`, so a shifted list still resolves (`agent/learning_mutations.py::_resolve_fingerprint`); edits/deletes go through `MemoryStore._mutate` **with the id re-resolved inside the lock** and exact-text match, refusing stale targets. Compaction interplay: pruned skills leave `[SKILL_PRUNED]` placeholders, and the safety rule instructs the model to reload via `skill_view(name='…')`.

## Prompt Assembly

`agent/prompt_builder.py` is stateless builders; `AIAgent._build_system_prompt()` combines them with memory + ephemeral blocks; `agent/system_prompt.py` resolves slots (e.g. the help-guidance variant is emitted only when `skill_view` exists). Verified order:

1. `DEFAULT_AGENT_IDENTITY` — written as a behavior spec ("models UNDER-explore by default; never re-add an exploration-thrift line")
2. Platform hints
3. Context files — `SOUL.md`, `AGENTS.md`, `.cursorrules`, `.hermes.md`/`HERMES.md` found by `_find_hermes_md` walking cwd→git root (cwd-only without git, with the comment "a file planted in /tmp") — each read on a daemon thread with a 5s timeout (`_read_text_with_timeout`) and **scanned for prompt injection** (`_scan_context_content` → `tools/threat_patterns.scan_for_threats`, scope "context"; project files blocked, user-authored SOUL.md warn-only)
4. Skills index
5. `build_memory_guidance`
6. `SKILLS_GUIDANCE` + Skill Safety Rule
7. `KANBAN_GUIDANCE`
8. Provider `system_prompt_block()`

Cache planning: `agent/prompt_caching.py::build_prompt_cache_plan` + `prompt_cache_boundary.py::register_stable_prefix`.

## Context Compression

`agent/context_compressor.py`: a cheap auxiliary model summarizes middle turns while head and tail are protected — iterative summaries, token-budget tail, **tool-output pruning first**, scaled budgets. Structure: `PreLlmSkipMixin` (skip if a pre-LLM check suffices), `SummaryDispatchMixin`, `CompressionTelemetryMixin`, `MicroCompactionMixin` (cheap targeted eviction, distinct from full compaction).

Decisions worth copying:

- The summary is wrapped in `SUMMARY_PREFIX`, a long "REFERENCE ONLY / latest message WINS / reverse signals end in-flight work / memory stays authoritative / tools remain fully active" preamble with `HISTORICAL_TASK_HEADING` — forged against observed failure modes (post-compaction narration-only turns, stale-task resumption).
- Guards: `finish_reason=="length"` → `_TRUNCATED_SUMMARY_MARKER` (a partial summary must not become a checkpoint); refusal detection via `_SUMMARY_REFUSAL_PREFIX_RE` + provider `message.refusal`, requiring the absence of `## ` headings to classify; quota/credential classification (`_is_summary_access_or_quota_error`).
- Route pinning: `pin_summary_route` is a **ContextVar**, not an attribute — the stalled worker is detached and still shares the compressor object; a one-shot pin survives exactly one retry, and `DETERMINISTIC_SUMMARY_ROUTE` swaps in a deterministic fallback summary.
- Anti-thrash re-arm: `_should_rearm_compression_budget` requires a completed-compaction latch **plus** a normalized prompt count below threshold — rough estimates cannot re-arm compression on their own.

Siblings: `conversation_compression_checkpoint.py` (durable checkpoints), `conversation_compression_reply_anchor.py`, `compression_marker.py` (`elide_middle`, `ELISION_MARKER_MAX_LEN`), `context_compressor_tail.py::demote_stale_tail_tools` (stale tool results → stubs; `skill_view_names_by_call_id` produces the `[SKILL_PRUNED]` stubs), `agent/image_eviction_policy.py`, `agent/iteration_budget.py`, `turn_context_compaction.py`.

## Tool Dispatch

Registry on the agent (`agent.tools`); schema assembly and distribution gating in `toolsets.py`/`toolset_distributions.py`; execution via `agent/tool_executor.py` with `tool_dispatch_helpers.py`, `tool_guardrails.py` (approval gates — e.g. writes to SOUL.md go through `tools/file_tools_write_guards.py`), `turn_tool_validation.py`, `tool_result_classification.py`, `tool_call_batches.py` (parallel batches), `inline_tool_executors.py`. Provenance: `tools/skill_provenance.py::set_current_write_origin` tags file writes with their origin.

Subagents: `subagent_lifecycle.py`, `managed_worker.py`, `delegation_context.py`. Multi-agent coordination via kanban (`~/.hermes/kanban.db`, `kanban_show/heartbeat/block/complete/request_review` tools, dispatcher reclaims work after `dispatch_stale_timeout_seconds`, workspace isolation via `$HERMES_KANBAN_WORKSPACE`).

## Crown Jewels (transferable to McHarness)

1. **Marker-gated incremental FTS backfill** (`hermes_state_fts.py` triggers + `state_meta` high-water/progress pair). Live writes and a background rebuild coexist; never index a row the rebuild hasn't reached; never fail writes when the tokenizer is absent (drop triggers + stale breadcrumb). → Rust re-implementation in the memory strata: same marker scheme over SQLite FTS5.
2. **Cache-boundary registry** (`prompt_cache_boundary.register_stable_prefix` + `append_user_instruction`). One construction site guarantees a byte-prefix invariant so the cache planner can split volatile suffixes. → `whole/prompt.rs`: KV-cache-aware prompt segmenting with registered stable prefixes.
3. **Compaction summary preamble as contract** (`SUMMARY_PREFIX`). Treat the summary as an inert handoff: legislate resumption behavior, reverse signals, memory precedence in the preamble itself. Cheap, high-leverage prompt engineering. → `whole/compaction.rs`.
4. **ContextVar route pinning** (`pin_summary_route`). A one-shot override that survives a retry on a *different thread* but cannot leak into sibling calls. → task-local route pinning for any concurrent fallback in Rust (tokio task-local).
5. **Fingerprinted node ids** (`learning_mutations.py`). `source:index:digest` ids survive list mutation; re-resolve by content *inside* the write lock and refuse on drift — optimistic concurrency for shared mutable stores. → memory strata write paths.
6. **Read-probe schema healing** (`schema_read_probe_statements`). Derive staleness probes from the schema DDL itself, table-qualified to dodge SQLite's string-literal misfeature. → schema versioning in the Rust store.
7. **Multi-slot scan cache keyed by full identity** (platform, home, project) with an on-disk signature — invalidation by *environment change*, not just explicit reload. → `fusion/discovery.rs` skill scanning.
8. **Failure-shape classifiers**: refusal/truncation detection before accepting an LLM summary as a checkpoint (`_is_summary_refusal`, `_TRUNCATED_SUMMARY_MARKER`). → `whole/failure.rs` taxonomy.

## What We Do NOT Copy

The **file-count explosion**: ~400 single-purpose `agent/` modules (60+ `hermes_state_*.py` mixins, 25+ `turn_*.py` phases) with heavy cross-imports and issue-number archaeology in comments — great for forensic maintenance, hostile to comprehension. The pinned-whitespace SQL strings (v22 DDL byte-pinned for a trace harness) and the regex-based refusal/trivial-prompt classifiers are brittle. The kanban/curator/review-fork surface assumes Nous's whole gateway stack; a Rust harness needs the *invariants* (locks, fingerprints, markers), not the machinery.
