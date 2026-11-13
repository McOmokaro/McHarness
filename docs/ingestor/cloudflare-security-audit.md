# Cloudflare Security-Audit Ingestor Dossier

**Lineage:** Cloudflare `security-audit-skill` — `cloudflare/security-audit-skill`, MIT License. A coding-agent *skill* (SKILL.md + Markdown companions + two zero-dependency Node.js validators), not a runtime service. Commit `c1c8a8c1471069fb0e188eeaff69b8e8db6564a8` (2026-09-14, "Clarify guidance and full audit modes"), cloned `--depth 1` into `forge/security-audit-skill`.
**Status:** `extracting` → `forged`
**Forge target:** `mch-gate` (schema/coverage validators + completion gates) and `mch-cyber` (the finding record + evidence bundle). This is the skill that seeded Cloudflare's fleet-wide vulnerability harness (per `README.md`); the repo is the single-repo ancestor of that harness.

---

## What I read, and how

The whole artifact is **22 files, ~5,400 lines**, all under `forge/security-audit-skill/skills/security-audit/`:

- `SKILL.md` (192 lines) — modes, sandbox/execution policy, write isolation, budget gate, severity anchors, the six-phase workflow, anti-patterns. The contract.
- Phase files: `RECONNAISSANCE.md` (156), `HUNTING.md` (251), `VALIDATION-AND-REPORTING.md` (186) — Phases 1–6.
- `ATTACK-CLASSES.md` (130) — the ordinary attack-class catalogue.
- Ten **domain companions** (73–105 lines each): `AI-AND-LLM.md`, `ATTACK-CLASSES.md`, `CLIENT-SIDE.md`, `CLOUD-AND-DEPLOYMENT.md`, `DATA-ISOLATION-AND-LIFECYCLE.md`, `DESKTOP-MOBILE-AND-LOCAL-IPC.md`, `MEMORY-SAFETY-AND-BINARY.md`, `PROTOCOLS-RPC-AND-MESSAGING.md`, `RESOURCE-EXHAUSTION-AND-AVAILABILITY.md`, `SUPPLY-CHAIN-AND-RELEASE.md`, `WEB-PROTOCOL-AND-AUTH.md`.
- **Machine contracts + tooling**: `report-schema.json` (461), `validate-findings.cjs` (773), `validate-coverage-ledger.cjs` (872), plus their tests (`validate-findings.test.cjs` 652, `validate-coverage-ledger.test.cjs` 740).
- `README.md` (106), `LICENSE` (MIT).

Three threads are load-bearing, and everything below is pinned to a file:line I read:

- **The finding schema** — `report-schema.json`, a strict three-verdict `oneOf` with `additionalProperties:false`.
- **The coverage ledger** — a deterministic coverage-identity scheme, enforced by `validate-coverage-ledger.cjs`.
- **The execution/evidence boundary** — the OS-sandbox rules and the descriptor-based artifact-promotion procedure, duplicated verbatim across `SKILL.md`, `HUNTING.md`, and `VALIDATION-AND-REPORTING.md`.

Where I extrapolate into Rust, I say so. The skill is prose + JSON + JS; there is no build, no server, no business logic to reverse.

---

## Architecture

The artifact is an **agent-neutral orchestration protocol**, not code. It says: a coordinating **parent** agent drives six phases, delegating to isolated typed sub-agents via the platform's "Task tool", and every result is written to fixed file names that two validators check.

```
parent agent (owns all shared state)
  │  Task tool  ──▶  `research` agents   (read source, return facts, write nothing)
  │             ──▶  `general`  agents   (hunt / verify; may run bounded local checks)
  │
  ├─ PHASE 1  Reconnaissance ───────────▶ architecture.md, coverage-ledger.json
  ├─ PHASE 2  Coverage-led hunting waves ─▶ candidate records (fingerprinted)
  ├─ PHASE 3  Candidate validation ──────▶ independent verifier per candidate
  ├─ PHASE 4  Structured output ─────────▶ findings.json  ── validate-findings.cjs
  ├─ PHASE 5  Independent record verify ─▶ fresh verifier per final record
  └─ PHASE 6  Target-neutral reports ────▶ REPORT.md, FINDINGS-DETAIL.md, NEEDS-VALIDATION.md
```

**Six phases** are named verbatim in `SKILL.md:168-178` and expanded in the phase files:

1. **Reconnaissance** (`RECONNAISSANCE.md`) — four parallel `research` agents map product/stack, principals/authority, entry surfaces/sinks, and local execution visibility; the parent synthesizes `architecture.md` (~1,000-word cap) and seeds a **deterministic coverage ledger**.
2. **Coverage-led hunting waves** (`HUNTING.md`) — the parent assigns `planned` ledger units to `general` hunters, each with a fully-specified verbatim prompt; a **coverage critic** (`research`) runs after every wave to find gaps.
3. **Candidate validation** (`VALIDATION-AND-REPORTING.md:3-93`) — every unique candidate goes to a fresh `general` verifier that *did not hunt it* and tries to refute it.
4. **Structured output** — parent writes `findings.json` (sorted by fingerprint), then runs both validators.
5. **Independent record verification** — one fresh `research` verifier per final `confirmed`/`needs_validation` record; material replacements get *another* independent verifier.
6. **Target-neutral reporting** — `REPORT.md`, `FINDINGS-DETAIL.md`, `NEEDS-VALIDATION.md` derived from the records, "with no live-probe instructions".

**Two operating modes** (`SKILL.md:10-17`): *guidance* (answer questions, no files, no full workflow) vs *full audit* (all six phases, write artifacts). Loading the skill does **not** authorize the full workflow — an important gate for a Rust re-implementation.

**Write isolation** (`SKILL.md:54-84`) is the security spine. The parent is the *only* writer of `run-metadata.json`, `architecture.md`, `coverage-ledger.json`, `findings.json`, `REPORT.md`, `FINDINGS-DETAIL.md`, `NEEDS-VALIDATION.md`. Each hunter/verifier gets `<output-dir>/agents/<agent-id>/{scratch,artifacts}/`; agents and target code may write **only** to `scratch/`. `artifacts/` is parent-owned and promoted by trusted code only. Agent IDs must match `^[a-z0-9][a-z0-9_-]{0,63}$` and must not be Windows device names.

**The budget gate** (`SKILL.md:119-134`) makes spend countable: one ledger unit ≈ one hunter, one candidate ≈ one or two verifiers. Before any reconnaissance agent launches, the parent reserves critic + validation calls; if the budget cannot fund them, it launches *nothing* and records `run_status:"incomplete"` with a machine reason (`budget_cannot_fund_reconnaissance_and_reserves`, `critic_budget_exhausted`, `validation_budget_exhausted`).

---

## Crown Jewels

### 1. `report-schema.json` — the three-verdict finding contract

The machine-readable vulnerability schema is a top-level **array** whose items are a `oneOf` over three branches, discriminated by the `verdict` field (`report-schema.json:1-461`). Every branch sets `additionalProperties:false`, so the contract is total — no stray fields. Verdicts are deliberately non-overlapping (`SKILL.md:150-152`, `VALIDATION-AND-REPORTING.md:103-107`):

| Verdict | Required fields (besides `verdict`) | Forbidden fields (enforced in code) |
|---|---|---|
| `confirmed` | `fingerprint`, `title`, `description`, `root_cause`, `intended_behavior`, `trace[]`, `evidence[]`, `conditions[]`, `execution`, `remediation`, `severity`, `confidence` | `claimed_root_cause`, `blockers`, `validation_plan`, `reason` |
| `needs_validation` | `fingerprint`, `title`, `description`, `claimed_root_cause`, `trace[]`, `evidence[]`, `blockers[]`, `validation_plan` | `severity`, `execution`, `remediation`, `reason`, `root_cause` |
| `rejected` | `fingerprint`, `title`, `description`, `claimed_root_cause`, `trace[]`, `evidence[]`, `reason` | `severity`, `execution`, `remediation`, `blockers`, `validation_plan`, `root_cause` |

Salient field types:

- **`fingerprint`** — the identity key. Pattern `^[A-Za-z0-9][A-Za-z0-9._:/@+-]*$` (`report-schema.json:17`); must be **stable across verdict states** for the same root cause and must not embed line/wave/agent/severity (`HUNTING.md:153-156`). Duplicates are an error and records must be **sorted lexicographically** by fingerprint (`validate-findings.cjs:496-506`).
- **`trace[]`** — ordered `{kind, file, line, scope, description}` where `kind ∈ {entrypoint, propagation, sink}`. The validator enforces the shape: a multi-step trace **starts** `entrypoint`, **ends** `sink`, all middles `propagation`; a one-step trace is `entrypoint` or `sink` (`validate-findings.cjs:528-546`).
- **`evidence[]`** — `{file, line, description}`, each `file` a **safe repository-relative path** (no absolute, `~`, `..`, backslash, colon, Windows device name — `validate-findings.cjs:473-483`).
- **`conditions[]`** — `{kind, description}` with `kind ∈ {authentication_level, authorization_role, user_interaction, system_configuration, network_routing, environmental_dependency, data_state, timing_dependency, third_party_dependency}` (`report-schema.json:107`).
- **`execution`** — `{attacker_perspective, payloads[], instructions[], observed_result}`; target-neutral, native interface (HTTP/CLI/library/message/fixture/browser/policy). `observed_result` must be nonempty visible prose.
- **`remediation`** — `{strategy, code_changes[]}`, each change `{file_name, fixed_code}`; "smallest effective source fix at the last trusted decision point" (`SKILL.md:164-166`).
- **`severity`** — `{likelihood:{score,reason}, impact:{score,reason}, overall_severity}` with score enum `informational|low|medium|high|critical`. Hard rule: **overall_severity cannot exceed demonstrated impact** — enforced by rank comparison (`validate-findings.cjs:559-563`; anchors at `SKILL.md:154-162`).
- **`confidence`** — `{score, reason}`, score enum `low|medium|high`.
- **`validation_plan`** (needs_validation only) — `{local?, deployment?}`; at least one must be present and nonempty visible prose (`validate-findings.cjs:569-574`). `deployment` is an *owner-observed* check, never live probing.

### 2. `coverage-ledger.json` + deterministic coverage identity

Coverage is not prose — it is a JSON array of units with a **canonical, reproducible ID** (`RECONNAISSANCE.md:90-137`). Each unit is one material combination of `surface × boundary × subsystem × attack_class` (+ optional `lifecycle`). The `coverage_id` is derived losslessly (`validate-coverage-ledger.cjs:354-375`):

1. Require each canonical ref to be NFC, visible, no control/format/default-ignorable code points.
2. Percent-encode its UTF-8 bytes with RFC 3986 (unescaped set `A-Za-z0-9-._~`, uppercase `%HH`).
3. Join `surface::boundary::subsystem::attack_class[::lifecycle]`.

This is a genuine crown jewel: **coverage has a content-addressed identity**, so two runs over the same source produce the same IDs and a validator can reject "one semantic tuple represented by different canonical references" (`validate-coverage-ledger.cjs:794-799`). The `quick` profile pins the coarsened dimension to the literal `profile/quick/all-in-scope-subsystems` (`RECONNAISSANCE.md:102`).

Each unit carries an exact **state table** (`RECONNAISSANCE.md:141-152`, enforced at `validate-coverage-ledger.cjs:518-566`): `planned` (no owner, empty evidence), `in_progress`, `blocked` (owner + partial evidence + nonempty `unresolved`), `covered` (owner + paths + checks, no unresolved), `candidate` (owner + paths + checks + `result_fingerprints`), and unassigned `not_applicable`/`out_of_scope`/`deferred`. `attempts[]` is an append-only archive for reopened units (critic reassignment), with **fresh owner** and un-reusable artifacts across waves (`validate-coverage-ledger.cjs:568-644`).

The unit also records its **assignment block map**: `ordinary_attack_class_block`, `selected_companion_blocks[]`, `excluded_blocks[{block, reason}]` — a machine-checkable link from a coverage obligation to the exact attack-class text the hunter received (`RECONNAISSANCE.md:104-135`).

### 3. The two zero-dependency validators (the tooling)

`validate-findings.cjs` and `validate-coverage-ledger.cjs` are self-contained Node CLIs that **interpret `report-schema.json` directly** (`validate-findings.cjs:1-9`) — no `ajv`, no deps. They are worth studying as much as anything else:

- **A hand-rolled JSON-Schema interpreter** supporting `type, const, enum, oneOf, required, properties, items, additionalProperties, minItems, minLength, minimum, pattern, uniqueItems` + a custom `visibleContent` keyword (`validate-findings.cjs:17-34,361-459`).
- **Defense-in-depth input handling**: files are opened `O_RDONLY|O_NOFOLLOW|O_NONBLOCK`, `fstat`-checked as a regular file under a 5 MiB cap, read from the descriptor, UTF-8-`fatal`-decoded (`validate-findings.cjs:604-649`); a byte-level **JSON preflight** enforces nesting/array limits *before* `JSON.parse` (`validate-findings.cjs:651-701`, `validate-coverage-ledger.cjs:141-275`).
- **Bounded everything**: `LIMITS = {inputBytes:5MiB, nestingDepth:64, arrayItems:1000, validationErrors:100, ...}`; the findings validator caps at 1,000 findings, the ledger at 10,000 units; diagnostics are escaped and clipped to 256 chars (`validate-findings.cjs:42-55`, `validate-coverage-ledger.cjs:13-25`).
- **Semantic checks beyond schema**: file-path safety, trace-kind ordering, severity≤impact, cross-record fingerprint dedup+sort, the `additionalProperties:false` forbidden-field matrix, and canonical-ID recomputation.
- **Exit codes**: `PASS: N findings valid` / `PASS: N coverage units valid` on success; `FAIL: N validation error(s)`; usage error returns 1 (`validate-findings.cjs:750-757`).

These two files are the exact pattern `mch-gate` needs, and both ship with their own test suites (`*.test.cjs`, Node `node:test`) that use the validators as libraries plus hostile-input CLI fixtures.

### 4. Sandbox + descriptor-based artifact promotion

The execution boundary is specified to the syscall (`SKILL.md:31-42`, copied **verbatim** into `HUNTING.md:99-136` and `VALIDATION-AND-REPORTING.md:48-85` as one identical fenced block). Target-controlled builds/tests/fuzzers may run only inside an OS-enforced sandbox with: no external network (isolated loopback only for a local fixture), an empty allowlisted environment with scratch-local `HOME`/caches, read-only target/toolchain, scratch-only writes, and explicit CPU/memory/process/file-size/disk/wall-clock limits. **If any control is missing, do not execute — return `needs_validation`.**

Promotion of scratch evidence is a race-safe, no-follow procedure: the parent holds non-inheritable dir fds for `scratch/` and `artifacts/`, walks each path component directory-relative with no-follow, opens the leaf `O_NOFOLLOW|O_NONBLOCK`, `fstat`-verifies regular file + link count 1 + byte bounds, copies exactly the verified size, re-`fstat`s, and creates the destination exclusively without following links. Never glob/recursively copy; never promote a symlink, FIFO, socket, device, hard link, or changing file. A local `check` is valid evidence **only** as a parent-promoted regular file under `agents/<check.agent_id>/artifacts/` (`RECONNAISSANCE.md:152`, `validate-coverage-ledger.cjs:394-398,441-449`).

### 5. Adversarial, independent validation (the model)

The organizing invariant: **the agent that checks a finding is never the agent that found it** (`README.md:95`). Phase 3 gives every candidate to a fresh verifier that receives the candidate, its linked checks, and prior records *with the same fingerprint* — but **not** another verifier's conclusion (`VALIDATION-AND-REPORTING.md:7`). Phase 5 gives every final record to *another* fresh verifier; a material replacement (any promotion, or a change to root cause/trace/execution/impact/severity) requires a *third* independent verifier before it can be applied (`VALIDATION-AND-REPORTING.md:145`). A malformed or prose-wrapped verifier result is **discarded, never repaired** (`VALIDATION-AND-REPORTING.md:89`).

Only `confirmed` records receive severity; `needs_validation` "means a specific source-grounded boundary hypothesis is blocked, not a low-confidence confirmed vulnerability, and it has no severity" (`SKILL.md:152`). Ten explicit anti-patterns (`SKILL.md:181-192`) include severity-on-`needs_validation`, checklist deviations as vulnerabilities, and writing the report before verification.

### 6. The attack-class taxonomy

`ATTACK-CLASSES.md` gives nine **ordinary** classes (Injection, Access control, Resource/file handling, Cryptography/secrets, Business logic, Feature abuse/data leakage, Chained vulnerabilities, Wildcard, Obvious things), each tagged `(subagent_type: general)`. Ten **companion** files add domain classes in a fixed shape — `#### When to use this file`, `## Core discipline`, named `**Class**` blocks, `## Universal moves`, `## Validation rules` — e.g. `MEMORY-SAFETY-AND-BINARY.md` (OOB, integer/unit confusion, UAF, type confusion, FFI/ABI layout, loader trust, kernel user-copy), `AI-AND-LLM.md` (indirect injection, memory poisoning, tool-arg injection, confused-deputy, MCP identity, output rendering). Companions are selected by *boundary found in reconnaissance*, not by language/dependency name (`RECONNAISSANCE.md:88`).

---

## Rust Re-implementation Targets

We do **not** port the Markdown (it is the prompt corpus) nor a runtime — there is none. We take the **schema contracts, the coverage-identity scheme, the validator semantics, and the evidence/promotion boundary** and land them in `mch-gate` and `mch-cyber`. `mch-gate` today (`crates/mch-gate/src/lib.rs`) is only ~246 lines: a `CompletionGate` of `Requirement`s (`ArtifactExists`, `ArtifactContains`, `DimensionCovered`, `ForbiddenSubstitute`) evaluated deterministically against a `RunContext` and a root `Path`. That is the correct home for the Cloudflare validators. `mch-cyber` already owns `hunt::Finding`/`FindingSeverity`/`HuntPhase` (`crates/mch-cyber/src/hunt.rs`) and the `evidence::EvidenceBundle` chain-of-custody (`crates/mch-cyber/src/evidence.rs`).

### `mch-gate` — port the two validators as native schema + coverage gates

| Cloudflare source | Location | `mch-gate` target |
|---|---|---|
| `report-schema.json` three-verdict `oneOf`, `additionalProperties:false` | `skills/security-audit/report-schema.json:1-461` | a `FindingSchema` serde model with `#[serde(tag="verdict", rename_all="snake_case")]` enum `{Confirmed, NeedsValidation, Rejected}`, `deny_unknown_fields`, and a `validate()` returning ranked errors — the native form of `validate-findings.cjs` |
| fingerprint pattern + dedup + lexical sort | `validate-findings.cjs:496-506` | enforce `^[A-Za-z0-9][A-Za-z0-9._:/@+-]*$` and stable-across-states identity in `FindingSchema::validate` |
| trace-kind ordering (entrypoint→propagation*→sink) | `validate-findings.cjs:528-546` | `enum TraceKind` + an ordered-trace invariant check |
| overall_severity ≤ impact rank comparison | `validate-findings.cjs:35-41,559-563` | a `Severity` `Ord` and a gate `Requirement::SeverityNotAboveImpact` |
| safe repository-relative path check | `validate-findings.cjs:473-483` | `fn is_safe_rel_path(&Path)` (shared with `mch-scope`'s existing path logic) |
| coverage-ledger JSON array + canonical ID `surface::boundary::subsystem::attack_class` | `RECONNAISSANCE.md:90-137`, `validate-coverage-ledger.cjs:354-375` | a `CoverageLedger`/`CoverageUnit` model with a `canonical_coverage_id()` (RFC 3986 percent-encode + `::` join) and `Status` enum matching the state table |
| ledger state-table invariants (owner/evidence per status) | `validate-coverage-ledger.cjs:518-566` | `CoverageUnit::validate_state()`; new `Requirement::LedgerState(...)` variants |
| bounded JSON preflight (nesting/array/byte caps, no-follow read) | `validate-findings.cjs:604-701` | `mch-gate::loader` — `serde_json` with a size-cap + `deny_unknown_fields`; the O_NOFOLLOW/`fstat` bound belongs in `mch-cyber::sandbox`, not the parser |
| the two `*.test.cjs` suites (valid + hostile-input fixtures) | `validate-findings.test.cjs`, `validate-coverage-ledger.test.cjs` | Rust integration-test fixtures ported 1:1 as the conformance corpus |

Reuse the existing `GateOutcome { Passed, Failed(BTreeSet<String>) }` (`mch-gate/src/lib.rs:53-69`) as the return type; the Cloudflare validators' "capped error list, non-zero exit on any error" maps directly onto it.

### `mch-cyber` — the finding record and the evidence bundle

| Cloudflare source | Location | `mch-cyber` target |
|---|---|---|
| `confirmed` record (root_cause, trace, execution, remediation, severity, confidence) | `report-schema.json:9-245` | extend `hunt::Finding` (`crates/mch-cyber/src/hunt.rs`) with target-neutral `Execution`, `Remediation`, `Confidence`, and a `Trace` of typed steps |
| `fingerprint` as stable identity | `report-schema.json:14-18` | `Finding.fingerprint: String` — the dedup/carry-forward key across runs |
| `evidence[]` (`{file,line,description}`) + `execution.payloads/observed_result` | `report-schema.json:74-153` | `evidence::EvidenceItem` (`crates/mch-cyber/src/evidence.rs:28-45`) already carries `{evidence_type, source, data, data_hash, collector}`; add a `SourceLocation {file,line,description}` variant |
| chain-of-custody + Ed25519 attestation of the bundle | `crates/mch-cyber/src/evidence.rs:73-135,255-286` | the Cloudflare "parent-only promotion + immutable record" rule is the same idea; keep `EvidenceBundle.verify()` as the integrity gate |
| six phases (Recon → Hunt → Validate → Output → Verify → Report) | `SKILL.md:168-178` | the `hunt::HuntPhase` state machine (`crates/mch-cyber/src/hunt.rs:97-135`) is adjacent but its own 9-phase pipeline; add the missing **independent-validation** and **coverage-ledger** phases as the Cloudflare contribution |
| sandbox controls + descriptor-based promotion procedure | `SKILL.md:31-42`, `HUNTING.md:99-136` | `crates/mch-cyber/src/sandbox.rs` (`Sandbox`, `DockerSandbox`, `CapabilityToken`) — add the no-follow, path-confined, regular-file, bounded-size promotion routine as a reusable `Promoter` |
| two operating modes + budget gate (launch nothing if reserves unfunded) | `SKILL.md:10-17,119-134` | a `RunMode`/`Budget` guard on the orchestrator; fail closed to `status:"incomplete"` with a machine reason |
| adversarial independence (verifier ≠ hunter; material change ⇒ third verifier) | `VALIDATION-AND-REPORTING.md:7,145` | encode as orchestration policy in `swarm`/`consensus`; the `consensus::Adjudicator` (`crates/mch-cyber/src/consensus.rs`) is the natural enforcement point |

`mch-gate` should own the **format + ledger-consistency gates** (a Rust `validate-findings`/`validate-coverage-ledger`); `mch-cyber` should own the **records produced** (`Finding`, `EvidenceBundle`) that those gates check. The `cmd_gate` command already gates on `findings.json` existing (`crates/mch-core/src/main.rs:111-126`) — extend it to gate on the findings *validating*, not merely existing.

---

## What I take, and what I leave

**Take:**
1. **The three-verdict finding schema** (`report-schema.json`) as the canonical `Finding` contract — a discriminated union where `confirmed`/`needs_validation`/`rejected` have disjoint, `additionalProperties:false` field sets, and only `confirmed` carries severity.
2. **Deterministic coverage identity** — RFC 3986-encoded `surface::boundary::subsystem::attack_class` IDs, so coverage claims are content-addressed and reproducible across runs.
3. **The validator semantics** (`validate-findings.cjs`, `validate-coverage-ledger.cjs`): trace-kind ordering, severity≤impact rank, fingerprint dedup+sort, physical-path safety, bounded pre-parse JSON limits — a template for `mch-gate`.
4. **The evidence/promotion boundary**: OS-sandbox-only execution and the no-follow, `fstat`-verified, size-bounded artifact-promotion procedure.
5. **Adversarial independence policy**: verifier ≠ hunter, and a third verifier for any material replacement.
6. **The budget/terminal-state discipline**: `run_status:incomplete` with a machine reason beats a silently thinned pass; exactly two terminal states.
7. **The attack-class taxonomy shape** (ordinary classes + domain companions with `Core discipline`/`Universal moves`/`Validation rules`) as a data model for hunter prompts — text stays as the prompt corpus.

**Leave:**
1. **The Markdown prose itself.** It is the LLM prompt corpus and belongs in `mch-cyber`'s prompt/asset layer, not as Rust logic.
2. **`research` vs `general` as platform names.** `SKILL.md` explicitly says these are role labels for "equivalent platform capabilities"; `mch-cyber`'s own agent roles supersede them.
3. **The Windows-device-name and Unicode-default-ignorable hardening *as a Rust gate stage*.** Important for a Node CLI reading hostile input; in Rust, `deny_unknown_fields` + a byte cap + the existing `mch-scope` path logic cover the same ground more cheaply. Keep the *checks* (path safety, boundaries), drop the ceremony.
4. **`node`/`.cjs` as the toolchain.** The validators become Rust; the `*.test.cjs` fixtures are kept only as a conformance corpus.
5. **Manual six-phase bookkeeping as file writes.** The durable artifact is the `findings.json`-shaped record inside `mch-cyber::evidence::EvidenceBundle`; separate `REPORT.md`/`FINDINGS-DETAIL.md` files become render functions, not state.

The value in this skill is not a scanner and not an agent framework — it is the **contract that makes AI security findings machine-checkable, deterministic, and independently verified**: a strict verdict schema, a content-addressed coverage ledger, and two validators that refuse to pass anything else. `mch-gate` is where those gates belong; `mch-cyber` is where the records they check are born.

---

## Files Reference

| File | Description |
|------|-------------|
| `forge/security-audit-skill/skills/security-audit/SKILL.md` | Modes, sandbox policy, write isolation, budget gate, severity anchors, six-phase workflow, anti-patterns |
| `forge/security-audit-skill/skills/security-audit/RECONNAISSANCE.md` | Phase 1: recon agents, architecture summary, deterministic coverage ledger + canonical ID scheme |
| `forge/security-audit-skill/skills/security-audit/HUNTING.md` | Phase 2: hunter prompts, promotion block, structured hunter result, coverage-critic waves |
| `forge/security-audit-skill/skills/security-audit/VALIDATION-AND-REPORTING.md` | Phases 3–6: candidate verification, `findings.json`, record verification, reports |
| `forge/security-audit-skill/skills/security-audit/ATTACK-CLASSES.md` | Ordinary attack-class catalogue + companion selection routing |
| `forge/security-audit-skill/skills/security-audit/MEMORY-SAFETY-AND-BINARY.md` | Companion: memory-safety, FFI/ABI, loader, kernel classes |
| `forge/security-audit-skill/skills/security-audit/AI-AND-LLM.md` | Companion: prompt injection, memory poisoning, tool/MCP trust |
| `forge/security-audit-skill/skills/security-audit/WEB-PROTOCOL-AND-AUTH.md` | Companion: HTTP framing/cache, sessions, JWT/OAuth/SAML, API keys/mTLS |
| `forge/security-audit-skill/skills/security-audit/CLIENT-SIDE.md` | Companion: DOM XSS, prototype pollution, CORS/messaging, service workers |
| `forge/security-audit-skill/skills/security-audit/SUPPLY-CHAIN-AND-RELEASE.md` | Companion: dependency/CI/release/signing/update classes |
| `forge/security-audit-skill/skills/security-audit/CLOUD-AND-DEPLOYMENT.md` | Companion: IAM, IaC, container, serverless, ingress, config |
| `forge/security-audit-skill/skills/security-audit/PROTOCOLS-RPC-AND-MESSAGING.md` | Companion: RPC/broker/queue/webhook/streaming classes |
| `forge/security-audit-skill/skills/security-audit/RESOURCE-EXHAUSTION-AND-AVAILABILITY.md` | Companion: amplification, accumulation, quota, recovery classes |
| `forge/security-audit-skill/skills/security-audit/DATA-ISOLATION-AND-LIFECYCLE.md` | Companion: tenant isolation, derived data, export/migration, deletion |
| `forge/security-audit-skill/skills/security-audit/DESKTOP-MOBILE-AND-LOCAL-IPC.md` | Companion: deep links, webview bridge, IPC, privileged helpers |
| `forge/security-audit-skill/skills/security-audit/report-schema.json` | Machine-readable finding schema (three-verdict `oneOf`) |
| `forge/security-audit-skill/skills/security-audit/validate-findings.cjs` | Zero-dependency findings validator (schema interpreter + semantic checks) |
| `forge/security-audit-skill/skills/security-audit/validate-coverage-ledger.cjs` | Zero-dependency coverage-ledger + canonical-ID validator |
| `forge/security-audit-skill/skills/security-audit/validate-findings.test.cjs` | Findings-validator conformance corpus |
| `forge/security-audit-skill/skills/security-audit/validate-coverage-ledger.test.cjs` | Coverage-validator conformance corpus |
| `forge/security-audit-skill/README.md` | Six-phase overview, install, design principles, provenance to Cloudflare's harness |
| `crates/mch-gate/src/lib.rs` | Rust target: `CompletionGate`, `Requirement`, `RunContext`, `GateOutcome` |
| `crates/mch-cyber/src/hunt.rs` | Rust target: `Finding`, `FindingSeverity`, `HuntPhase` state machine |
| `crates/mch-cyber/src/evidence.rs` | Rust target: `EvidenceBundle`, `ChainOfCustody`, `Attestation` |
| `crates/mch-cyber/src/sandbox.rs` | Rust target: sandbox controls + promotion procedure |
| `crates/mch-core/src/main.rs` | `cmd_gate` — currently gates on `findings.json` existence (extend to validity) |
| `docs/ingestor/cloudflare-security-audit.md` | This document |
