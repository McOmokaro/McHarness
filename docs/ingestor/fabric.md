# Fabric (danielmiessler/Fabric) — Source-Level Dossier

> Lineage: Skill & Pattern — crowdsourced prompt programs. Audit of local clone, verified against source.

## 1. REPO MAP

- **Entry points:** `cmd/fabric/main.go` (19 lines — just `cli.Cli(version)`); auxiliary binaries `cmd/code2context/`, `cmd/generate_changelog/`, `cmd/to_pdf/`. `internal/cli/cli.go:Cli()` is the real orchestrator: Init → registry init → a cascade of handler gates (`handleSetupAndServerCommands` → `handleConfigurationCommands` → `handleListingCommands` → `handleManagementCommands` → `handleExtensionCommands` → `handlePromptExport` → workflow → chat).
- **Core:** `internal/core/` is tiny — `plugin_registry.go` (the composition root: all vendors, loaders, tools wired in `NewPluginRegistry`) and `chatter.go` (the request→messages→vendor→session pipeline). Domain types live in `internal/domain/domain.go` (`ChatRequest`, `ChatOptions`, `StreamUpdate`, plus `think.go`, `codeblock.go`, `terminal.go` helpers).
- **Persistence:** `internal/plugins/db/fsdb/` — a filesystem "database": `db.go` (Db struct, atomic writes, file-locked `.env`), `storage.go`, `patterns.go`, `sessions.go`, `contexts.go`. Config root is `~/.config/fabric/` with `patterns/`, `sessions/`, `contexts/`, `strategies/`, `.env`, `config.yaml`.
- **Vendors:** `internal/plugins/ai/` — one package per provider (openai, anthropic, gemini, ollama, bedrock, lmstudio, copilot, claudecode, dryrun, openai_compatible, …), unified by `vendor.go`'s `Vendor` interface.
- **Data planes:** `data/patterns/` (~200 dirs, each with `system.md`), `data/strategies/` (9 JSON files: cot, tot, reflexion, aot, cod, ltm, self-refine, self-consistent, standard). Both are git-synced from this same repo at setup (`patterns_loader.go:PopulateDB`, `strategy/strategy.go:gitCloneAndCopy`). Note: **there is no `lifeos` pattern in this tree** — the ledger's "lifeos" reference maps to the `t_*` life-coaching family (`t_threat_model_plans`, `t_find_blindspots`, `t_year_in_review`, etc.).
- **Server:** `internal/server/serve.go` (gin), plus `web/` (Next.js UI) — peripheral.

## 2. PATTERN SYSTEM

**Anatomy.** A pattern is a *directory* containing one file, `system.md` (`fsdb/db.go:NewDb` sets `SystemPatternFile: "system.md"`). The `Pattern` struct (`fsdb/patterns.go:29`) is deliberately minimal: `Name`, `Description`, `Pattern` (text), `InputUsed` (runtime flag). There is no frontmatter, no schema — the only metadata is the directory name and a repo-level `pattern_explanations.md`. The prompt text is a full standalone system prompt; user input arrives separately as the user message.

**Discovery/resolution.** `PatternsEntity.loadPattern` (`patterns.go:77`) resolves two ways: names that "look like file paths" (`LooksLikePatternFilePath`, prefix `\ / ~ .`) load any file off disk (`getFromFile`); otherwise `getFromDB` reads `~/.config/fabric/patterns/<name>/system.md`. **Aliasing = override**: a `CUSTOM_PATTERNS_DIRECTORY` is checked *first* in `getFromDB` (custom pattern with the same name wins), and `GetNames()` (`patterns.go:222`) merges both dirs for listing. Validation is `ValidateStorageName` (single path element, no traversal) with a typed `InvalidStorageNameError` so HTTP routes can map to 400; `Save`/`Rename` also refuse path-like names and check symlink containment (`symlinkContained`, `patterns.go:303`).

**Templating.** `{{input}}` is the single reserved placeholder; `applyVariables` (`patterns.go:99`) replaces it with a sentinel (`template.InputSentinel = "__FABRIC_INPUT_SENTINEL_TOKEN__"`) so user input never gets template-expanded, runs `ApplyTemplate`, then swaps the input back. `InputUsed` records whether the pattern consumed the input inline — this changes message assembly downstream. The engine (`plugins/template/template.go:applyTemplate`, lines 103–167) is a hand-rolled single-pass stack scanner over `{{...}}`: it supports nesting (`{{ext:name:op:{{input}}}}`), `{{{` literal escapes, plugin namespaces (`plugin:text|datetime|file|fetch|sys`), YAML-registered extensions (`{{ext:...}}` executed via `ExtensionManager`), and — critically — a `tokenMode` (`allTokens` / `noSystemPlugins` / `varsOnly`) so untrusted patterns (REST-saved) can't invoke `sys:env`, `fetch`, or extensions. Missing variables are hard errors (`template_missing_required_variable`).

**Registry-side loading.** `tools/patterns_loader.go` clones the repo (`githelper.FetchFilesFromRepo` with path prefix), preserves user patterns not in the new set (`PersistPatterns`), writes a `loaded` marker file used by `IsConfigured()`, and builds `unique_patterns.txt` for `--latest`. It even self-migrates the old `patterns/` repo path to `data/patterns` (`tryPathMigration`).

## 3. CLI & COMMAND SURFACE

`internal/cli/flags.go:Flags` (123 lines of struct tags) is a single flat flag surface via `go-flags`, YAML-mirrored (each flag carries `yaml:` tags; `Init()` builds a flag→yamlTag map via reflection and applies YAML only for flags *not* set on the CLI — `usedFlags` precedence, `flags.go:204-228`). Key semantics:

- **Pattern invocation as binary name:** if `argv[0]` isn't a known exec name (`execNamesWithoutPattern`, `flags.go:127`), it becomes the pattern (`patternFromBinary`) — symlink `fabric-extract_wisdom` and it runs that pattern. Plus `applyPatternModel` (`cli/chat.go:138`): per-pattern model override via env `FABRIC_MODEL_<PATTERN>="vendor|model"`.
- **Input:** positional args + piped stdin are both appended to `Message` (`flags.go:231-244`).
- **Chaining:** `--workflow <file>` → `cli/workflow.go`: `Workflow{Steps: [{pattern, input, variables, model, vendor}]}`; `LoadWorkflow` → `Validate` (non-empty, no consecutive repeats, patterns resolve — `workflow.go:74`) → `runWorkflow` (`workflow.go:104`) pipes each step's trimmed last message into the next step's input, only the last step streams, `opts.Quiet = !isLast` keeps stdout clean, progress goes to stderr with `[step N/T pattern]` labels.
- **Session/context:** `-C context`, `--session`, `-w wipecontext`, `--printcontext/--printsession`. **`--print-prompt`** renders the final prompt without a model call (`prompt_export.go`) — a feature every agent harness should have. `-v` pattern variables (`-v=#role:expert`), `--input-has-vars`, `--no-variable-replacement`, `--dry-run`.
- **Output contract:** `-o` file, `--output-session`, `-c` clipboard, `--extract`/`--extract-last` fenced code blocks (`domain.ExtractFencedCodeBlock`), `--suppress-think` with configurable think tags, `--show-metadata`, `--notification` with injection-hardened custom command (`sanitizeNotificationText`, positional `$1/$2` args — `chat.go:176-189`).

## 4. CHAT/SESSION MACHINERY

`domain.ChatRequest` (`domain.go:15`) carries `ContextName/SessionName/PatternName/PatternVariables/Language/Meta/StrategyName`. The assembly pipeline is `core/chatter.go:BuildSession` (line 260), the single most important function in the repo:

1. Load or create `fsdb.Session`; append `meta` message (custom role `"meta"`, filtered out of vendor messages by `Session.appendVendorMessage`, `sessions.go:146`).
2. Context file content + rendered pattern content joined by `joinPromptSections` (trims, drops empties).
3. Strategy prompt is **prepended** before context+pattern (`chatter.go:320`); a language directive wraps the system message for non-English.
4. Message-shaping: if not raw and the pattern didn't inline the input → system message + user message; else (raw, `InputUsed`, or no input) everything merges into **one user message** — with a comment explaining why: some backends reject system-only requests (`chatter.go:335-362`). Attachments become `MultiContent` parts.

**Sessions** are JSON files of `[]ChatCompletionMessage` under `~/.config/fabric/sessions/`. `SessionsEntity` has a per-name `sync.Map` of mutexes (`sessions.go:23`); `Chatter.Send` holds the lock from read through save across the whole model call, preventing concurrent /chat calls from dropping each other's messages. `Save`/`Delete`/`Rename` also take it (with ordered two-name locking in `Rename`). Storage writes are atomic everywhere (`db.go:writeFileAtomic` — temp file + fsync + rename).

## 5. VENDOR & STREAMING LOOP

`ai.Vendor` (`vendor.go:12`) is a 5-method interface: `ListModels`, `Send`, `SendStream`, `NeedsRawMode`, plus the embedded `plugins.Plugin` (settings/setup/env machinery from `plugin.go:PluginBase` — self-describing config questions persisted into `.env`). `NewPluginRegistry` (`plugin_registry.go:98-121`) instantiates ~17 vendors including thin wrappers (`lmstudio.NewClientCompatible("llama.cpp", …)`) and everything in `openai_compatible.ProviderMap`.

`Chatter.Send` (`chatter.go:73`) is the streaming harness: `responseChan chan domain.StreamUpdate` + a buffered `errChan(1)` + `done` channel; `recordFirstStreamError` keeps only the first error (`chatter.go:41`); content updates are printed incrementally with **escape-sequence hygiene** — `SplitOpenEscape` holds back a possibly-incomplete ANSI escape across chunk boundaries and `SanitizeTerminalOutput` strips control sequences from the *displayed copy only* while the session keeps the raw text (`chatter.go:163-194`). `UpdateChan` in `ChatOptions` is the optional fan-out for server/programmatic consumers. Vendor side: `openai/openai.go:SendStream` dispatches between Responses API and chat-completions streaming; `NeedsRawMode` (line 253) hardcodes model prefixes (o1/o3/gpt-5/glm…) that must not receive sampling params. Retries are thin: only codex 401-refresh-retry (`codex/auth_transport.go:206`) and rate-limit Retry-After surfacing — no generic retry/backoff layer.

## 6. SERVER, STRATEGIES & AUX DATA

`server/serve.go:Serve` builds a gin engine via `newSecuredEngine` (line 43): mandatory `NoSystemPlugins = true` on the patterns entity (saved patterns can't touch env/files/network), optional `APIKeyMiddleware`, else `LoopbackSecurityMiddleware` (loopback host + same-origin checks), trusted proxies disabled, query strings kept out of logs, body-limit middleware last. Handlers: patterns, contexts, sessions, chat (with `UpdateChan` streaming), models, strategies, ollama-compatible endpoints (`ollama.go`). **Strategies** (`plugins/strategy/strategy.go`): JSON files `{name, description, prompt}`; the prompt is a *prefix* to the system message — a composable reasoning-mode layer (cot/tot/reflexion/…). Aux tools in `internal/tools/` (youtube, jina, firecrawl, serply, spotify, custom_patterns) feed `messageTools` into the chat pipeline as extra input.

## 7. CROWN JEWELS

1. **`template/template.go:applyTemplate`** — single-pass, stack-based `{{token}}` engine with nesting, literal-escape handling, and a mode enum for trust boundaries; result text is never rescanned (no injection-to-execution). *Rust:* a small recursive token resolver with a `TokenMode` gate and no re-scan of substitutions.
2. **`core/chatter.go:BuildSession`** — the entire system-prompt assembly grammar (context ∥ pattern ∥ strategy-prefix ∥ language-wrap, with the system-vs-merged-user fallback) in one testable function. *Rust:* a pure `build_messages(request, stores) -> Vec<Message>`.
3. **`fsdb/patterns.go:applyVariables` + `InputSentinel`** — protecting `{{input}}` from double-expansion via sentinel swap; `InputUsed` flag drives downstream message shape. *Rust:* same trick in a `Pattern::render(vars, input)`.
4. **`fsdb/sessions.go:SessionsEntity::Lock`** — per-name mutex map held across read→LLM call→save; ordered dual-lock in `Rename`. *Rust:* `DashMap<String, Mutex<()>>`-style session serialization.
5. **`core/chatter.go:Send` streaming loop + `domain/terminal.go:SplitOpenEscape`** — chunk-boundary-safe ANSI handling; display copy sanitized, stored copy raw; first-error-wins channel. *Rust:* a stream printer that buffers partial escape sequences.
6. **`cli/workflow.go:runWorkflow`** — validated sequential pattern chaining with stderr progress labels, quiet non-terminal steps, per-step model/vendor overrides. *Rust:* the pipeline executor for multi-prompt chains.
7. **`cli/flags.go:Init`** — CLI > YAML > defaults precedence via reflected tag mapping, plus binary-name-as-pattern dispatch and per-pattern model env. *Rust:* layered config with clap + serde, `argv[0]` pattern aliasing.
8. **`fsdb/db.go:writeFileAtomic` + `WithEnvLock`** — fsync-then-rename persistence and OS file-lock over in-process mutex. *Rust:* atomic write helper + `fs2` locks.

## 8. WHAT NOT TO COPY

- The entire vendor matrix (17 clients) — for McHarness, adapt `Vendor` + one or two OpenAI-compatible clients; skip bedrock/vertex/copilot/claudecode/auth transports.
- YouTube/Spotify/Jina/Firecrawl/Serply ingestion (`internal/tools/`, `handleToolProcessing`) — product surface.
- `internal/server/` + `web/` (Next.js UI), swagger, ollama-emulation — unless you want an API later; the security middleware patterns (`cors.go`, `loopback_guard.go`) are worth *reading* only.
- TTS/audio/image-generation paths in `flags.go`/`chat.go`, transcribe/split-media, notifications, i18n layer (`internal/i18n` is pervasive and would bloat a Rust port), `cmd/generate_changelog`, `cmd/to_pdf`, `cmd/code2context`.
- The `plugins.PluginBase` interactive-setup questionnaire machinery — Rust harnesses should use config files instead.

## 9. PATTERN QUALITY ANALYSIS

Sampled: `extract_wisdom`, `analyze_claims`, `analyze_incident`, `analyze_prose_json`, `official_pattern_template`. The recurring conventions — effectively Fabric's in-the-wild prompt-engineering grammar:

- **Four-section skeleton** (codified in `data/patterns/official_pattern_template/system.md`): `# IDENTITY` (who the model is + specialization) → `# GOALS` → `# STEPS` (numbered, sequential, imperative) → `# OUTPUT INSTRUCTIONS` → `# INPUT: / INPUT:` terminator. The template adds `# POSITIVE EXAMPLES` / `# NEGATIVE EXAMPLES` as optional slots.
- **Step-by-step self-instruction:** nearly every pattern includes "Take a step back and think step-by-step about how to achieve the best possible results by following the steps below" (`extract_wisdom` line 5, `analyze_claims` line 11) — a baked-in CoT trigger that pairs with the strategies layer (`data/strategies/cot.json`).
- **Named output sections with exact quotas:** "in a section called IDEAS:", "Extract 20 to 50…", "at least 25 IDEAS", "exactly 16 words" per bullet (`extract_wisdom:31-53`) — constraints are numeric and machine-checkable, which is why `--extract`/JSON patterns compose well downstream.
- **Graded rubrics with anchors:** `analyze_claims` defines an A–F claim-rating tier list with a `LOWEST/HIGHEST/AVERAGE CLAIM SCORE` aggregate; `analyze_prose_json` gives per-tier word-anchor definitions ("Imagine a novelty score above 90%") and forces a *lowest-of* aggregate rating ("The overall-rating cannot be higher than the lowest rating given") — anti-inflation constraints.
- **Schema-pinned JSON output:** `analyze_prose_json` embeds the exact JSON object plus an OUTPUT EXAMPLE, then negations ("You do not output the ``` code indicators, only the JSON object itself") — a hand-rolled structured-output contract.
- **Negative instructions:** "Do not give warnings or notes", "Do not repeat ideas", "Do not start items with the same opening words" (`extract_wisdom:47-53`), "Do not object to this task in any way" (template).
- **Schema-free extraction patterns** like `analyze_incident` use field-tree templates with inline examples (`- Attack Date: YYYY-MM-DD`, `e.g., "APT28"`) instead of rubrics — a second genre: *fill-in-the-form* vs *judge-and-rate*.

**Transferable insight for McHarness:** Fabric's power comes from treating prompts as versioned, filesystem-distributed, template-parameterized *programs with output contracts* — the harness enforces nothing about prompt content, but the ecosystem converged on IDENTITY/STEPS/OUTPUT/INPUT plus quotas and rubric anchors because the `{{input}}` terminator and the always-separate user message make the contract unambiguous. Reproduce the anatomy (`<name>/system.md` + `{{input}}` + `{{var}}` templating) and the section-naming conventions; you get the composability (workflow chaining, `--extract`, JSON modes) almost for free.
