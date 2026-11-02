# Hermes Agent Ingestor Dossier

**Lineage:** Hermes Agent (NousResearch/hermes-agent) — MIT-ish, Python
**Status:** `extracting` → `forged`
**Forge target:** `mch-memory` (Rust), wired into `mch-core`

---

## What I read, and how

I cloned the repo (1.6 GB, ~22k files) into `forge/hermes-agent` and read source, not READMEs. The agent loop here is ordinary — the crown jewels are three subsystems nobody copies well: how it turns a session into a skill, how it keeps memory from rotting, and how it improves itself while you sleep.

Three threads, all load-bearing:

- **Skill-Sealing** — `tools/skill_manager_tool.py`, `agent/learn_prompt.py`, `tools/skill_usage.py`, `tools/skill_ledger.py`
- **Memory** — `tools/memory_tool.py`, `tools/memory_tool_store.py`, `agent/memory_manager.py`, `agent/memory_provider.py`
- **Learning loop** — `agent/curator.py`, `agent/learning_graph.py`

Everything below is a claim I can point at a file for. Where I extrapolate, I say so.

---

## 1. Skill-Sealing — experience becomes procedural memory

The tool is `skill_manage`, declared in `tools/skill_manager_tool.py`. One sentence in its own docstring fixes the mental model:

> "Skills are the agent's procedural memory (narrow 'how to do X'; MEMORY.md/USER.md are broad, declarative)."

That split is the whole design. Memory holds facts that apply to every turn; a skill holds a procedure that applies when its trigger matches. The failure mode it avoids: dumping procedures into memory and bloating every prompt with them forever.

**On-disk shape.** `<skills>/[category/]<skill>/SKILL.md`, plus optional `references/`, `templates/`, `scripts/`, `assets/`. The directories are not cosmetic — `skill_view` discovers them relative to the skill root, and the curator's consolidation rules key on them.

**One call, one operations array.** The advertised schema has a single shape: `operations`, an ordered list of ops. Each op is a tagged union (`_op_schema`), and every branch sets `additionalProperties: false`. The comment on that flag is the interesting part:

> "'additionalProperties: false' is what lets a grammar-constrained backend refuse another action's text slot outright."

The old flat shape (`action`, `content`, `old_string` at top level) is still accepted for replaying old transcripts, but never advertised. A comment records why: with one object holding `content` and `file_content` side by side, "a 27B model that just used write_file kept emitting file_content on create and the whole batch rolled back (#112677)." That is a real failure caught in production and fixed by narrowing the schema, not by prompting harder.

**The mutation is a critical section.** `skill_manage` runs under `_skill_mutation_lock(name)`, which locks `.locks/<sha256(basename)>.lock` — never inside the skill dir, so a delete/recreate cannot unlink a lock out from under a waiting writer. A batch acquires every lock "in one stable path order (deadlock-free across batches)."

Inside that window, in order: capture the pre-mutation state for the ledger, run the handler, and on success record the side effects. If the handler is refused or the security scan blocks, nothing is left half-written.

**Read-before-write is a gate, not advice.** `_guarded_write` refuses a patch or overwrite of an existing file unless the caller read it this turn. The curator prompt states it flat: "A write without that read is REFUSED and nothing is saved." This is the guard against an agent that patches from a stale copy of a file another session changed.

**The security scan is opt-in and rollback-safe.** With `skills.guard_agent_created` on, a new or edited skill is scanned (`scan_skill`); a dangerous verdict restores the original bytes (or unlinks a new file) and returns the report so the agent can retry without the offending pattern. Off by default because `terminal` runs the same code ungated — the scan is a speed bump for the careless path, not a sandbox.

**Every mutation is ledgered.** `_record_success` writes an append-only JSONL entry through `tools/skill_ledger.py` with before/after blobs, and tags the actor. A `delete` archives rather than `rmtree`s when it comes from the background review, because "Archives are recoverable; deletion is not." After any write it clears the skills system-prompt cache so the next turn sees the new skill.

**The description budget is a routing constraint, not style.** The frontmatter `description` must fit `SKILL_PROMPT_DESC_LIMIT` (60 chars). The authoring standards in `agent/learn_prompt.py` explain why, and it is the sharpest paragraph in the repo:

> "the system-prompt skill index truncates the description to 60 chars and loads it every session, so anything past char 60 is silently cut and never routes. After you write the description, COUNT the characters; if it is over 60, cut it down."

An over-long description does not fail loudly. It just stops matching. `_description_budget_error` enforces the limit at write time so the failure becomes visible.

**How a session becomes a skill.** `agent/learn_prompt.py::build_learn_prompt` builds the one prompt the live agent runs for `/learn`. It has no distillation engine and no hidden model call — the docstring says so: "No distillation engine, no model-tool footprint, so it works identically on local, Docker, and remote backends." The agent gathers sources with the tools it already has, then authors the skill with `skill_manage`. Two shapes are named by source size: a workflow gets one tight `SKILL.md`; a book or doc corpus gets the knowledge-base layout — a lean always-loaded `SKILL.md` index plus per-chapter `references/` files loaded on demand. And the source text is data, never instructions: `_SOURCE_HYGIENE` drops bidirectional and zero-width Unicode before distilling, which closes the Trojan-Source hole where a document reads one way to you and another to the agent.

### Rust target — `mch-memory::skill`

| Piece | Python | Rust |
|---|---|---|
| Tagged-union op set | `_op_schema`, `_ACTION_HANDLERS` | `enum SkillOp { Create, Patch, WriteFile, RemoveFile, Delete }` |
| Per-skill critical section | `_skill_mutation_lock` | `SkillLock` (advisory `.lock`, `O_EXCL` + retry) |
| Read-before-write gate | `_guarded_write` | `edit_requires_prior_read()` on the store |
| Description budget | `_description_budget_error` | `const SKILL_INDEX_DESC_LIMIT: usize = 60` |
| Bounded body | `_validate_content_size` | `const MAX_SKILL_BODY: usize = 100_000` |
| Provenance ledger | `skill_ledger.record_mutation` | append-only `SkillLedger` (JSONL) |

---

## 2. The three strata of memory

Hermes has three kinds of memory, and they are not three tables in one store. They are three subsystems with different lifetimes, costs, and write rules. Getting the boundaries right is the extraction.

### Stratum A — persistent, declarative memory

Files: `tools/memory_tool.py`, `tools/memory_tool_store.py`. Two files under `~/.hermes/memories/`: `MEMORY.md` (the agent's own notes) and `USER.md` (who the user is). Entries are joined by `ENTRY_DELIMITER = "\n§\n"`.

The design decision that matters is the frozen snapshot. From the module docstring:

> "Both enter the system prompt as a FROZEN snapshot at session start; mid-session writes hit disk but never change the prompt (prefix cache intact)."

The store freezes `_system_prompt_snapshot` when it loads the files. A write later in the session lands on disk and is visible to the next session, but the current session's prompt text never changes. That keeps the provider prefix cache valid for the whole run — the same trick OpenCode uses for its compaction cache, applied to memory. It also means an agent cannot reason about memory it just wrote mid-turn; it must finish the write and move on.

Budgets are in characters, not tokens: `memory_char_limit` 2200, `user_char_limit` 1375 by default. Small on purpose — this text rides in every turn. The `memory` tool and `skill_manage` both explain the split: "Anything learned while doing a task... belongs in the task's skill... memory is injected into every turn and must stay small."

Writes are atomic batches. The limit is checked only on the final result:

> "a single call can remove/patch stale entries to free room AND add new ones, even when an add alone would overflow."

That is the mechanism that stops memory from filling up and staying full. An at-capacity add returns the current entries and tells the model to reissue one batch that prunes and adds together. After three failed consolidation attempts in a turn, `_MAX_CONSOLIDATION_FAILURES_PER_TURN` returns a terminal result so a stubborn add cannot loop the turn to budget exhaustion and swallow the user's reply.

Two guards sit under the write path. `_scan_memory_content` runs a strict threat scan, because "memory enters the system prompt, so a poisoned entry persists across sessions." And a drift guard refuses to flush when the file holds an entry larger than the whole limit (free text appended without delimiters) — rewriting would silently discard it, so the write is refused and a `.bak` snapshot is taken.

### Stratum B — pluggable provider memory

Files: `agent/memory_manager.py`, `agent/memory_provider.py`. The built-in store is one provider among possible others. At most one external provider is registered at a time (`add_provider` rejects a second). A provider is an ABC with a lifecycle: `initialize` → `prefetch`/`sync_turn` per turn → tool dispatch → `shutdown`, plus optional hooks (`on_turn_start`, `on_session_end`, `on_pre_compress`, `on_memory_write`, `on_delegation`).

Two details are worth stealing. First, **egress redaction on every fan-out**: `_redact_for_provider` scrubs secrets from everything handed to a provider, "because providers archive whatever they are handed." Second, **the stuck-call gate**: `_StuckCallGate` admits calls to a hung backend up to a bound, then closes and probes once a minute, so one wedged provider cannot pin a thread per fresh manager. Python cannot kill a thread, so the gate bounds the damage instead.

### Stratum C — the working transcript

The live conversation lives in `hermes_state_*.py` — `hermes_state_messages.py`, `hermes_state_compression.py`. This is working memory, and it is managed by compression, not by the memory tool. When the context fills, the parent session is closed with `end_reason='compression'` and a child row takes the transcript. The chain is walked by `get_compression_chain` / `get_compression_tip`. Locks (`compression_locks`) and turn leases (`session_turn_leases`) serialize who may rotate a conversation, so two agents cannot fork the same lineage. This is the same shape as OpenCode's compaction, and `mch-compaction` already covers the policy half.

### Rust target — `mch-memory::store`

| Piece | Python | Rust |
|---|---|---|
| Two-file store | `MemoryStore` | `struct MemoryStore { memory: Vec<String>, user: Vec<String> }` |
| Frozen prompt snapshot | `_system_prompt_snapshot` | `fn frozen_snapshot(&self) -> MemoryBlock` (captured at load) |
| Atomic batch vs final budget | `apply_batch` | `fn apply_batch(&mut self, target, ops) -> Result<()>` |
| Char budgets | `memory_char_limit`, `user_char_limit` | `const MEMORY_CHARS: usize = 2200; const USER_CHARS: usize = 1375` |
| Strict threat scan | `_scan_memory_content` | `fn scan_entry(&str) -> Option<Threat>` |
| Drift guard | `_drift_error` | `fn check_drift(&self) -> Option<Drift>` |
| Provider trait | `MemoryProvider` (ABC) | `trait MemoryProvider` with the same hook set |

---

## 3. The learning loop — the agent improves itself

Two files, two mechanisms. `agent/curator.py` is the maintenance pass; `agent/learning_graph.py` is what makes the learning visible.

**The curator runs on idleness, not a daemon.** The module docstring: "Inactivity-triggered (no cron daemon): when the agent is idle and the last run is older than `interval_hours`, `maybe_run_curator()` auto-transitions lifecycle states from activity timestamps." Defaults: interval 7 days, minimum idle 2 hours, stale after 14 days, archive after 30.

There are two passes, and the separation is deliberate.

The first is a **pure function with no model** — `apply_automatic_transitions`. Every curator-managed skill moves `active → stale → archived` on its latest real activity timestamp. No LLM. Crucially, "use_count == 0 is absence of evidence, not staleness: never archive a never-used skill younger than stale_after_days." A skill you have not needed yet is not dead. Pinned skills and cron-referenced skills are exempt.

The second pass is the **opt-in LLM consolidation fork** (`DEFAULT_CONSOLIDATE = False`). `CURATOR_REVIEW_PROMPT` is a long, opinionated brief whose thesis is worth quoting because it is the anti-pattern most skill libraries fall into:

> "A collection of hundreds of narrow skills where each one captures one session's specific bug is a FAILURE of the library — not a feature... One broad umbrella skill with labeled subsections beats five narrow siblings for discoverability."

The hard rules it enforces on the fork: never touch bundled/hub/external skills; never delete, only archive (recoverable); never touch pinned; never use `use_count` as a reason to keep OR prune; and — the one that keeps the whole thing auditable — "You have NO terminal access in this pass — every filesystem mutation goes through skill_manage... so it is ledgered and rollback-able." A shell `mv` writes the same bytes with no ledger entry, so the archive would snapshot a stripped package and rollback would restore a hollow skill. The constraint is the point.

Invariants stated at the top of the file: "only curator-managed skills are touched; never delete, only archive (recoverable); pinned skills bypass all auto-transitions; the fork uses the auxiliary client and never touches the main session's prompt cache."

**What counts as learning.** `agent/learning_graph.py::_has_learning_signal` defines it precisely: a skill is graph-worthy when `created_by in {"agent", "learn"}` or `use_count > 0`. External mounts are never a learning milestone — "they were configured by the user, not learned." The graph nodes are learned skills plus memory chunks (`MEMORY.md` / `USER.md` entries, each fingerprinted by the SHA-256 of its text so an edit does not collide with a neighbour). Edges come from declared `related_skills` and from lexical overlap between a memory card and skill names.

State lives in `~/.hermes/skills/.curator_state` (scheduler) and `.usage.json` (per-skill telemetry and lifecycle state). The telemetry is a sidecar, never frontmatter — "keeps telemetry out of user-authored SKILL.md and off bundled/hub skills." Curator management is an explicit `created_by: agent` marker written by `skill_manage`, "never inferred from location."

### Rust target — `mch-memory::curator`

| Piece | Python | Rust |
|---|---|---|
| Idle gate | `should_run_now` | `fn should_run(&self, now) -> bool` |
| Lifecycle states | `STATE_ACTIVE/STALE/ARCHIVED` | `enum SkillState { Active, Stale, Archived }` |
| Pure transitions | `apply_automatic_transitions` | `fn apply_transitions(&mut self, now) -> TransitionReport` |
| Learning signal | `_has_learning_signal` | `fn has_learning_signal(node) -> bool` |
| Learning graph | `build_learning_graph` | `fn build_graph(&self) -> LearningGraph` |
| Recoverable archive | `.archive/` move | `fn archive(&self, name) -> Result<()>` (move, never delete) |

---

## 4. File map

| Capability | File | Lines (approx) | McHarness crate |
|---|---|---|---|
| Skill sealing tool | `tools/skill_manager_tool.py` | 943 | `mch-memory::skill` |
| Skill authoring prompt | `agent/learn_prompt.py` | 197 | `mch-memory::skill` (constants) |
| Skill telemetry + states | `tools/skill_usage.py` | — | `mch-memory::curator` |
| Mutation ledger | `tools/skill_ledger.py` | — | `mch-memory::skill` |
| Memory tool | `tools/memory_tool.py` | 477 | `mch-memory::store` |
| Memory store | `tools/memory_tool_store.py` | 740 | `mch-memory::store` |
| Provider fan-out | `agent/memory_manager.py` | 935 | `mch-memory::provider` |
| Provider ABC | `agent/memory_provider.py` | 225 | `mch-memory::provider` |
| Curator | `agent/curator.py` | 1212 | `mch-memory::curator` |
| Learning graph | `agent/learning_graph.py` | 255 | `mch-memory::graph` |
| Working-memory compression | `hermes_state_compression.py` | 946 | `mch-compaction` (exists) |

---

## 5. What I take, and what I leave

**Take:**
1. The three-strata split, and the rule that decides which stratum a fact belongs to.
2. The frozen prompt snapshot for declarative memory — it is what lets the prefix cache survive a session.
3. Atomic batch writes checked against the final budget, with a per-turn failure cap.
4. The read-before-write gate on skill mutation, and the ledger that makes every change reversible.
5. The 60-char description budget as a routing constraint.
6. Recoverable archive instead of delete, everywhere.
7. `use_count == 0` is not staleness.
8. Source hygiene against bidi/zero-width characters before distilling a document.

**Leave:**
1. The Python-specific thread gates (`_StuckCallGate`) — Rust has `tokio::time::timeout` and real cancellation.
2. The multi-profile ContextVar machinery — McHarness ships a single-profile default.
3. `_scan_memory_content`'s regex threat set as-is; McHarness wants its own policy, and `mch-scope` already owns authorization.
4. The four-file `hermes_state_*` working-memory store; `mch-compaction` covers the policy and this crate stays out of the transcript.

The memory tool and the curator are where the self-improvement actually lives. The skill engine is the hinge: it is the only place experience becomes durable, and it is the only place the agent is allowed to write to its own future prompts — so it is the only place that earns a ledger and a lock.
