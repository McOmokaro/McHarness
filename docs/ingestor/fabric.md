# Fabric Ingestor Dossier

**Lineage:** Fabric (danielmiessler/fabric) — MIT, Go core, v1.4.568 (`de0b727`). A CLI + pattern library that applies reusable prompt "patterns" to arbitrary content (YouTube, PDFs, stdin).
**Status:** `extracting` → `forged`
**Forge target:** `mch-brain` (pattern/skill corpus), `mch-dsh-llm` + `mch-core` (provider abstraction), `mch-tui` (CLI surface), `mch-transport` (streaming).

---

## Architecture

Fabric is small in vocabulary and large in surface. Four nouns carry the whole system: **Plugin**, **Vendor**, **Pattern**, and **Chatter**. Everything is a plugin; every plugin is configured by env-var settings; one registry wires them together; one orchestrator (`Chatter`) turns a `ChatRequest` into a model call. I read source, not READMEs.

### The plugin spine

Every capability implements `plugins.Plugin` (`internal/plugins/plugin.go:19`):

```go
type Plugin interface {
    GetName() string
    GetSetupDescription() string
    IsConfigured() bool
    Configure() error
    Setup() error
    SetupFillEnvFileContent(*bytes.Buffer)
}
```

`PluginBase` (`plugin.go:28`) provides all six, plus an `EnvNamePrefix`, a list of `Settings`, and a list of `SetupQuestion`s. **A setting is an env var.** `AddSetting`/`AddSetupQuestion` derive the variable name from the plugin name (`BuildEnvVariable`, `plugin.go:364`) and mark credential-looking names (`KEY`, `TOKEN`, `SECRET`, `PASSWORD`) as `Sensitive` so they print masked (`isSensitiveName`, `plugin.go:163`; `MaskedValue`). `Configure()` reads the env file values into the settings and validates required ones; `Setup()` walks the interactive questions and writes answers back to `os.Setenv`. This is the single configuration mechanism — vendors, the pattern loader, strategies, and tools all use it.

`core.PluginRegistry` (`internal/core/plugin_registry.go:144`) is the composition root. `NewPluginRegistry` (`:54`) constructs every plugin — `PatternsLoader`, `CustomPatterns`, `Strategies`, `YouTube`, `Language`, `Jina`, `Serply`, `Firecrawl`, `Spotify`, `Defaults`, `TemplateExtensions` — and registers ~20 AI vendors through `VendorsAll.AddVendors(...)` (`:98`). It holds two vendor sets: `VendorsAll` (everything known) and `VendorManager` (only configured/active ones). `activateVendor` (`:470`) lazily promotes a vendor from all→active on first use and caches it.

### Patterns — the reusable prompt corpus

A pattern is a directory: `~/.config/fabric/patterns/<name>/system.md`. The `system.md` is the whole prompt (the repo ships 265 of them under `data/patterns/`, e.g. `data/patterns/extract_wisdom/system.md`, plus an `official_pattern_template/system.md` authoring skeleton). An empty `user.md` sits beside it as the conventional slot for caller input (51 dirs have one; the code only ever reads `system.md` — `SystemPatternFile`, `internal/plugins/db/fsdb/db.go:24`).

Storage is the filesystem, wrapped by `fsdb.PatternsEntity` (`internal/plugins/db/fsdb/patterns.go:15`). Discovery is `os.ReadDir` over the pattern dir (`storage.go:70`, `GetNames`); a custom-patterns directory can shadow the main one with the same name (`patterns.go:136`) and is merged into the listing (`GetNames`, `patterns.go:222`). There is **no per-pattern version field** — "versioning" is the git repo the corpus is cloned from (`internal/tools/patterns_loader.go`, `DefaultPatternsGitRepoUrl` = the fabric repo, folder `data/patterns`). `UpdatePatterns` (`-U`) re-clones into a temp dir and `PersistPatterns` (`patterns_loader.go:125`) copies back any local pattern the upstream does not have, so a user's custom patterns survive an update. A `loaded` marker file gates "is configured" (`:68`), and `unique_patterns.txt` is a sorted name cache (`createUniquePatternsFile`, `:297`).

Parameterization is where the real design lives: `GetApplyVariables` (`patterns.go:39`) loads the text then runs it through the template engine with the caller's `map[string]string` variables and the user `input`. `{{input}}` is special — it is swapped for a sentinel first so that variables are never expanded *inside user input* (`applyVariables`, `patterns.go:99`), then restored. This is the defense against a document that smuggles a `{{...}}` token into the prompt.

### The template engine — a one-pass, mode-gated token resolver

`internal/plugins/template/template.go` is the most carefully written file in the repo. `applyTemplate` (`:103`) walks the template **left to right in a single pass**, resolving `{{...}}` tokens and writing each result as literal text that is *never re-scanned* — so a resolved value cannot become a new token (comment block, `:63`). It handles nested tokens (`{{plugin:text:upper:{{name}}}}`) and Mustache-style `{{{...}}}`.

Crucially the resolver is **mode-gated** (`tokenMode`, `:92`):

- `allTokens` — `ApplyTemplate`: runs every plugin and extension.
- `noSystemPlugins` — `ApplyTemplateNoSystemPlugins`: only `text:` and `datetime:`; a `sys:`, `file:`, `fetch:` or `ext:` token is an **error**. This is what the REST server uses (`PatternsEntity.NoSystemPlugins`), because a client that saved a pattern must not be able to read the server env, read files, or fetch URLs (`patterns.go:21`).
- `varsOnly` — `ApplyTemplateInput`: no plugins at all; a plugin/extension token in user input stays literal.

Token syntax: `{{plugin:<namespace>:<op>:<value>}}` and `{{ext:<name>:<op>:<value>}}` (regexes, `:34`). `matchTriple` (`:43`) requires the match to span the *entire* token, so an injected closing brace cannot splice in a new token. Plugins are `text`, `datetime`, `file`, `fetch`, `sys` (`resolveToken`, `:202`).

### Extensions — verified external commands

Beyond built-in plugins, Fabric runs **user-registered external programs** as template functions. `ExtensionRegistry` (`internal/plugins/template/extension_registry.go`) stores YAML definitions (`~/.config/fabric/extensions/extensions.yaml`). Registration (`Register`, `:98`) computes a hash of the config **and** the executable and stores both. On every `GetExtension` (`:184`) the current hashes are recomputed and compared — a changed config or swapped binary is rejected (`extension_config_hash_mismatch` / `extension_executable_hash_mismatch`). Execution (`extension_executor.go`) fills the operation's `cmd_template` with **shell-escaped** values (`shellEscape`, `:91`) before `sh -c`, and the `file` output method runs under a `context.WithTimeout` (`:112`).

### Providers — one interface, one OpenAI-compatible base

The provider abstraction is a five-method interface (`internal/plugins/ai/vendor.go:12`):

```go
type Vendor interface {
    plugins.Plugin
    ListModels(context.Context) ([]string, error)
    SendStream(context.Context, []*chat.ChatCompletionMessage, *domain.ChatOptions, chan domain.StreamUpdate) error
    Send(context.Context, []*chat.ChatCompletionMessage, *domain.ChatOptions) (string, error)
    NeedsRawMode(modelName string) bool
}
```

`ai.VendorsManager` (`internal/plugins/ai/vendors.go:23`) is the registry: case-insensitive `FindByName`, an `AtomicU64`-guarded model catalog that fans out `ListModels` concurrently (`readModels`, `:88`), and a `sync.Mutex` around the cache. The catalog is a `VendorsModels` group model — models grouped by vendor, looked up case-insensitively, and used to translate a user's model spelling into the vendor's spelling (`GetChatter`, `plugin_registry.go:534`; `model:/vendor` split at `:613`).

The leverage is that most vendors are the **same client with different defaults**. `openai.Client` (`internal/plugins/ai/openai/openai.go`) exposes `NewClientCompatible(vendorName, baseURL, configureCustom)` (`:32`); DigitalOcean, LM Studio (and `llama.cpp`), exolab, perplexity, xAI, and the whole `openai_compatible.ProviderMap` (`plugin_registry.go:118`) are constructed from it. The client also abstracts the OpenAI **Responses API vs Chat Completions** split behind `ImplementsResponses` (`:70`, `supportsResponsesAPI`, `:280`), so callers see one `Send`. Anthropic, Gemini/Vertex, Bedrock, and the CLI-backed vendors (Codex, Claude Code, Copilot) are bespoke `Vendor` impls that fit the same interface. `NeedsRawMode` (`:284`) lets a vendor force raw prompts for models that reject sampling params.

### Chatter — the orchestrator

`core.Chatter` (`internal/core/chatter.go:23`) is the only thing that knows how a request becomes a conversation. `BuildSession` (`:266`) assembles the message list in a fixed order:

1. `meta` (the original argv, for reproducibility) as a `meta`-role message,
2. context text (`contexts/<name>`),
3. the rendered pattern text,
4. a strategy prompt **prepended** to the system message (`:326`),
5. a language-enforcement wrapper (`:336`),
6. then either (system message + user message) when the pattern has no `{{input}}`, or one merged user message when it does (`:343`–`:368`).

`Send` (`:73`) streams or blocks. In stream mode it spawns a goroutine calling `vendor.SendStream`, forwards `domain.StreamUpdate` values out of a channel, prints content deltas to stdout (with terminal-escape sanitizing and think-block suppression), and reports the *first* stream error only (`recordFirstStreamError`, `:41`). Non-stream mode calls `vendor.Send` and returns a string. Both wrap provider failures in `domain.ProviderError` (`internal/domain/stream.go:28`) so a provider message/text never leaks into structured logs unmasked.

### Request data flow (end to end)

A single `fabric -p extract_wisdom -m gpt-4o --stream < input.txt` travels:

```
argv → cli.Init (go-flags/YAML) → cli.Cli dispatch → handleChatProcessing
  → Flags.BuildChatRequest / BuildChatOptions → registry.GetChatter(model,vendor,...)
    → Chatter.Send → BuildSession:
         meta + context + [template.ApplyTemplate(pattern, vars, input)] + strategy + language
      → Session.GetVendorMessages()  (drops `meta`-role messages)
      → vendor.SendStream(...) ──goroutine──▶ chan domain.StreamUpdate ──▶ stdout
  → save session JSON (under per-name lock) → optional extract/output/clipboard/notify
```

The `meta`-role message is stored in the session for provenance but **excluded** from what the vendor sees (`appendVendorMessage`, `sessions.go:146`). A per-pattern model override (`FABRIC_MODEL_<PATTERN>` = `vendor|model`) is applied before the chatter is built (`applyPatternModel`, `chat.go:138`). REST requests enter the same path through `server/chat.go` but with `NoSystemPlugins` set, and every user-supplied pattern/context/session name is rejected if it looks like a file path (`rejectUnsafePatternName`).

### Sessions, contexts, and the env file — the persistence layer

`fsdb.Db` (`internal/plugins/db/fsdb/db.go:38`) holds three entities: `Patterns`, `Sessions` (JSON files under `sessions/`), `Contexts` (text under `contexts/`). Sessions get a **per-name mutex** (`SessionsEntity.locks`, `sessions.go:23`) held from read to save so two `/chat` calls on one session cannot lose each other's messages; renames lock both names in sorted order to avoid deadlock (`sessions.go:65`). Storage names are validated (`ValidateStorageName`, `storage.go:239`) against `.`/`..`, path separators, Windows reserved device names, and trailing dot/space; `resolvedPath` (`:295`) re-checks containment after absolute resolution and rejects symlinks that escape the directory (`symlinkContained`, `:266`). The `.env` file is written atomically (temp file + rename, `writeFileAtomic`, `db.go:180`) and under a cross-process file lock for env mutations (`WithEnvLock`, `db.go:126`).

### The CLI command model

`Flags` (`internal/cli/flags.go:32`) is a single struct of ~120 options parsed by `go-flags`; a `--config` YAML file can also supply them. `Cli` (`internal/cli/cli.go`) is a straight command dispatcher — the pattern is "each handler returns `handled bool`; if handled, exit":

```
handleSetupAndServerCommands → handleConfigurationCommands → handleListingCommands
  → handleManagementCommands → handleExtensionCommands → workflow → handleChatProcessing
```

`handleListingCommands` (`internal/cli/listing.go:19`) serves `-l/--listpatterns`, `--readpattern`, `-n/--latest`, `-L/--listmodels`, `-x/-X` contexts/sessions, `--liststrategies`, `--listvendors` — all writing plain text to stdout, with a `--shell-complete-list` mode for tab completion. `handleChatProcessing` (`internal/cli/chat.go:20`) is the run path: build `ChatRequest`/`ChatOptions`, send, then optionally extract a fenced code block (`--extract`), copy to clipboard, write to `--output`, send a desktop notification, or apply file changes (the `create_coding_feature` pattern, `chatter.go:234`). `handleWorkflowProcessing` (`internal/cli/workflow.go`) chains patterns: a YAML/JSON `Workflow` of steps where each step's output is the next step's input (`runWorkflow`, `:104`), only the last step streaming.

Streaming output exists in two transports over the same `domain.StreamUpdate` type (`internal/domain/stream.go`): CLI stdout in `Chatter.Send`, and **SSE** in `internal/server/chat.go` (`text/event-stream`, `StreamResponse{type,format,content,usage}`, route `POST /chat`). The REST server deliberately runs patterns with `NoSystemPlugins` and `rejectUnsafePatternName` so the CLI's file-path pattern feature never crosses the HTTP boundary (`patterns.go:70`, `server/chat.go`).

---

## Crown Jewels

- **The `Vendor` interface + `VendorsManager` registry** — a five-method provider contract (`internal/plugins/ai/vendor.go:12`) with a concurrent, self-invalidating model catalog (`internal/plugins/ai/vendors.go:23`).
- **The OpenAI-compatible base client** — `openai.NewClientCompatible` (`internal/plugins/ai/openai/openai.go:32`) plus the `openai_compatible.ProviderMap`, the mechanism by which ~a dozen vendors are "the same client with different defaults," and the Responses-vs-ChatCompletions abstraction behind one `Send`.
- **The pattern store contract** — `fsdb.PatternsEntity` (`internal/plugins/db/fsdb/patterns.go`): `loadPattern`/`getFromDB`/`getFromFile`, custom-dir shadowing, and the `{{input}}`-sentinel substitution that makes user input un-templatable.
- **The single-pass, mode-gated template engine** — `applyTemplate` and `tokenMode{allTokens,noSystemPlugins,varsOnly}` (`internal/plugins/template/template.go:92`–`:103`); no re-scan of resolved values, whole-token `matchTriple` matching, and a security mode that denies `sys`/`file`/`fetch`/`ext` to untrusted templates.
- **The hash-verified extension registry + shell-escaped executor** — config *and* executable hashing on every load (`internal/plugins/template/extension_registry.go:184`), single-quote argument escaping and a `context.WithTimeout` (`internal/plugins/template/extension_executor.go:91`,`:112`).
- **The `Chatter` prompt assembler** — the deterministic message order in `BuildSession` (`internal/core/chatter.go:266`): meta → context → pattern → strategy → language, plus the input-in-system-vs-user split and the "request must end with a user message" rule.
- **Strategy as a first-class prompt modifier** — `strategy.Strategy{name,description,prompt}` (`internal/plugins/strategy/strategy.go:58`) loaded from `data/strategies/*.json` and prepended to the system message (`chatter.go:326`); the CoT/ToT/self-consistency family.
- **Git-backed corpus refresh with local preservation** — `PatternsLoader.PopulateDB`/`PersistPatterns`/`tryPathMigration` (`internal/tools/patterns_loader.go:87`,`:125`,`:239`) and `unique_patterns.txt`; versioning by git-clone, not by record.
- **Filesystem storage hardening** — `ValidateStorageName`, `resolvedPath`, `symlinkContained`, `writeFileAtomic`, `WithEnvLock` (`internal/plugins/db/fsdb/storage.go`, `db.go`): path-traversal, symlink-escape, Windows-reserved-name, and partial-write defenses.
- **Per-name session locking** — `SessionsEntity.locks` (`internal/plugins/db/fsdb/sessions.go:23`) with read-to-save hold and sorted dual-name renames.
- **The `StreamUpdate` streaming contract** — `domain.StreamUpdate{Type,Content,Usage}` + `StreamTypeContent/Usage/Error` (`internal/domain/stream.go`) shared by CLI stdout and REST SSE, plus first-error-only capture (`recordFirstStreamError`).
- **The env-var-as-setting configuration model** — `plugins.Plugin` + `PluginBase.Settings`/`SetupQuestion` (`internal/plugins/plugin.go`) with automatic credential masking and `SetupFillEnvFileContent` round-trip.
- **The workflow chaining model** — `Workflow`/`WorkflowStep` and `runWorkflow` (`internal/cli/workflow.go:21`), where each pattern's output pipes into the next and only the last step streams.

---

## Rust Re-implementation Targets

| Jewel | Fabric source | McHarness crate |
|---|---|---|
| Pattern corpus as reusable, composable skills | `internal/plugins/db/fsdb/patterns.go`, `data/patterns/*/system.md` | **`mch-brain`** (`skills.rs` `SkillStore`/`SkillEntry`) — a pattern becomes a `SkillEntry`; the Fusion `SkillStore` is the corpus index |
| Single-pass, mode-gated template / variable expansion | `internal/plugins/template/template.go` | **`mch-brain`** (`skills.rs` rendering) + a `render` module — port `tokenMode` gating verbatim; it is prompt-injection defense |
| `Vendor` interface + registry + model catalog | `internal/plugins/ai/vendor.go`, `internal/plugins/ai/vendors.go` | **`mch-dsh-llm`** (`ModelStream`, `ProviderAdapter`, `ProviderRegistry`) — `Vendor` ≈ `ModelStream`, `VendorsManager` ≈ `ProviderRegistry` |
| OpenAI-compatible base client & provider map | `internal/plugins/ai/openai/openai.go`, `.../openai_compatible/` | **`mch-dsh-llm`** (`adapters::openai_compat`) — one `OpenAiCompatAdapter::with_base_url` reused per provider |
| Bring-your-own-key / env-var provider config | `internal/plugins/plugin.go` (`Setting`, `Sensitive`), `internal/plugins/db/fsdb/db.go` (`.env`) | **`mch-core`** (Key Farm: key registry + env-file writer) + **`mch-dsh-llm`** (`CredentialProvider`, `ApiKeyProvider`) |
| `Chatter` prompt assembly + stream orchestration | `internal/core/chatter.go` | **`mch-loop`** (`StepExecutor`) for the turn loop; **`mch-brain`** for message/skill assembly |
| Strategy (CoT/ToT/reflexion) as prompt modifier | `internal/plugins/strategy/strategy.go`, `data/strategies/*.json` | **`mch-brain`** (`cls.rs` / skill prelude) — a strategy is a system-message prefix |
| Streaming `StreamUpdate` contract + SSE | `internal/domain/stream.go`, `internal/server/chat.go` | **`mch-transport`** (`TransportConfig`, WS/HTTP fallback) + a `StreamChunk`-style delta type |
| Workflow chaining (output→input pipe) | `internal/cli/workflow.go` | **`mch-plan`** (`executor.rs`, `state.rs`) — an ordered step DAG with per-step model/vendor |
| CLI command dispatch surface | `internal/cli/flags.go`, `internal/cli/cli.go`, `listing.go` | **`mch-tui`** (`commands.rs`) — `-l/--read/--update`, `handled bool` dispatch |
| Hash-verified external command extensions | `internal/plugins/template/extension_registry.go`, `extension_executor.go` | **`mch-gate`** (verify-before-act) + **`mch-transport`**-adjacent exec sandbox; shell-escape + timeout are load-bearing |
| Filesystem storage hardening | `internal/plugins/db/fsdb/storage.go`, `db.go` | **`mch-memory`** (storage) + **`mch-scope`** (name/containment policy) — `ValidateStorageName` → scope authority |
| Session / context persistence + per-name locks | `internal/plugins/db/fsdb/sessions.go`, `contexts.go` | **`mch-memory`** (`sessions`, atomic JSON, per-key lock) |
| Git-backed corpus refresh w/ local preservation | `internal/tools/patterns_loader.go` | **`mch-brain`** (`curator.rs` consolidation) + a `fetch` module (preserve local, pull upstream) |

**Take:** the `Vendor`/`VendorsManager` split (all-known vs active, concurrent catalog with case-insensitive model resolution), the OpenAI-compatible base that collapses N vendors into one client, the `{{input}}` sentinel, and the whole template engine — especially `tokenMode`, which is a security boundary, not a feature. Take the git-clone-plus-preserve pattern refresh and the hash-verified extension registry.

**Leave:** pattern storage as a filesystem tree of `system.md` directories (McHarness stores skills in `mch-brain`, not on disk per-name); the `user.md` convention (unused by the code); and the ~120-field flat `Flags` struct (split into verb subcommands under `mch-tui`).

The value is the *contraction*: Fabric proves that a provider abstraction only needs five methods, that "different provider" is usually "same client, different base URL," and that prompt templates must be pruned by trust level at render time. Those three ideas are what `mch-dsh-llm`, `mch-core`, and `mch-brain` inherit.
