# DeepSeek Harness (DSH) Ingestor Dossier

**Lineage:** DeepSeek Harness (deepseek-ai/deepseek-harness) — MIT, TypeScript monorepo
**Status:** `extracting` → `forged`
**Forge target:** `mch-dsh` (Rust), wired into `mch-core` and `mch-loop`

---

## What I read, and how

I cloned the repo (20k+ files) into `forge/deepseek-harness` and read source, not READMEs. DSH is a TypeScript monorepo built on **Cordis** — a meta-framework that implements "everything is a plugin" at the kernel level. The crown jewels are three subsystems nobody copies well: the Cordis plugin kernel, the BYOK provider abstraction, and the Super-Agent routing layer.

Three threads, all load-bearing:

- **Cordis Kernel** — `vendor/cordis/src/` (fiber.ts, registry.ts, context.ts, events.ts)
- **BYOK Provider Abstraction** — `packages/llm/` (provider-neutral `llm` service, adapters)
- **Super-Agent Routing & Agent Model** — `packages/core/agent/` (`ctx.agents`, `agent.model`, `agent.model.select()`)
- **Dynamic Package Runtime** — `packages/extensions/cordis-client-runner/` + `cordis-host-runner/` (browser/host halves)

Everything below is a claim I can point at a file for. Where I extrapolate, I say so.

---

## 1. Cordis Kernel — "everything is a plugin"

The tool is `cordis`, declared in `vendor/cordis/`. One sentence in its own docstring fixes the mental model:

> "Cordis is a TypeScript plugin framework for applications that need explicit dependency injection, scoped services, lifecycle-managed cleanup, and optional configuration-driven loading."

That split is the whole design. The kernel is 8 files in `vendor/cordis/src/`:

- `context.ts` — root dependency container, `Context.extend()` for scoped children
- `fiber.ts` — lifecycle state machine (`PENDING` → `LOADING` → `ACTIVE` → `UNLOADING` → `DISPOSED`), effect system, config resolution, `_reload()`/`_unload()`
- `registry.ts` — `RegistryService`, plugin shape normalization (`Plugin.Function`/`Constructor`/`Object`), `plugin()`, `inject()`, `delete()`
- `events.ts` — waterfall/dispatch/emit, `internal/config`, `internal/update`, `internal/commit`
- `reflect.ts` — service reflection, `provide`/`inject`/`intercept`, `@Inject` decorator
- `service.ts` — `Service` base class, `symbols.init`/`initHooks`
- `logger.ts` — built-in logger with levels
- `utils.ts` — `DisposableList`, `symbols`, `buildOuterStack`

### On-disk shape

A plugin is any of three shapes, normalized in `registry.ts:resolve()`:

```ts
// Function plugin
(ctx: Context, config: T) => any

// Class plugin
new (ctx: Context, config: T) => any

// Object plugin
{ apply(ctx: Context, config: T): any }
```

All carry optional metadata in `Plugin.Base`: `name`, `Config` (standard-schema validator), `inject` (service deps), `provide` (service names), `intercept` (intercept config keys).

### The mutation is a critical section

`RegistryService.plugin()` (registry.ts:316) creates or reuses a `Runtime` record, then constructs a `Fiber` with validated config and resolved injections. The fiber's `dispose()` is the only teardown path — effects, provisions, and registry entries all unwind through it.

**Read-before-write is a gate, not advice.** `Fiber.update()` validates against the runtime's `Config` schema via `resolveConfig()` (fiber.ts:50) before applying. A failed validation throws `ValidationError` and the running instance is untouched.

**Every mutation is ledgered.** `fiber.update()` emits `internal/commit` → `internal/update` waterfall. `Fiber._effectiveConfig` freezes the successful candidate. Volatile updates (loader-driven) retain references without restarting.

### The description budget is a routing constraint, not style

Cordis doesn't enforce a character limit, but the **loader** (`@cordisjs/plugin-loader`) and **config-file includes** (`@cordisjs/plugin-include`) drive the pattern: config-driven loading from YAML/JSON means the plugin's `Config` schema *is* its documentation. A plugin with no `Config` schema loads with empty config; one with a 500-line schema self-documents.

### How a session becomes a plugin

In DSH, *every capability* is a Cordis plugin:

- `packages/llm/llm/` → `ctx.llm` (provider-neutral model call service)
- `packages/llm/llm-deepseek/` → DeepSeek adapter
- `packages/llm/llm-pi-ai/` → pi-ai catalog adapter
- `packages/mcp/mcp-client/` → one MCP server connection
- `packages/mcp/mcp-manager/` → fleet of `mcp-client` instances
- `packages/core/agent/` → `ctx.agents` (Agent registry + handle)
- `packages/core/agent-loop/` → `dsh-agent-loop` (driver factory)
- `packages/skill/` → `ctx.skills` (skill registry)
- `packages/extensions/cordis-client-runner/` + `cordis-host-runner/` → dynamic package runtime (browser/host halves)

### Rust target — `mch-dsh::cordis`

| Piece | TypeScript | Rust |
|---|---|---|
| Context container | `Context` | `struct Context { registry: Registry, intercept: HashMap, ... }` |
| Plugin shapes | `Plugin.Function`/`Constructor`/`Object` | `enum Plugin { Function(Box<dyn Fn(Context, Value)>), Constructor(...), Object(...)}` |
| Fiber lifecycle | `FiberState` enum + `Fiber` class | `enum FiberState { Pending, Loading, Active, Unloading, Disposed }` + `struct Fiber` |
| Plugin registry | `RegistryService` + `Runtime` map | `struct Registry { runtimes: HashMap<PluginKey, Runtime> }` |
| Effect system | `ctx.effect()` + `Disposable` | `trait Effect { fn run(&self) -> Disposable }` |
| Config validation | `StandardSchemaV1` + `resolveConfig()` | `trait ConfigSchema { fn validate(&self, Value) -> Result<Value> }` |
| Effect cleanup | `DisposableList` + `ctx.effect()` | `struct EffectGuard { disposables: Vec<Box<dyn FnOnce()>> }` |
| Hot reload | `@cordisjs/plugin-hmr` | `struct HmrManager { watch: Notify, reload: mpsc::Sender }` |
| Dependency injection | `inject` map + `ctx[Context.intercept]` | `struct InjectMap { deps: HashMap<String, Option<InterceptConfig>> }` |

---

## 2. BYOK Provider Abstraction — "Bring Your Own Key"

The `packages/llm/` group is a **provider-neutral model-call service** (`ctx.llm`). The architecture:

### Core vocabulary (`packages/llm/llm/`)

- `Message` — role + content blocks (text, image, tool calls, tool results)
- `ToolCall` / `ToolResult` — shared with session log
- `StreamChunk` — `TextDelta` | `ToolCall` | `ToolResult` | `Finish` | `Error`
- `ModelRequest` / `ModelResponse` — the wire shapes

### Provider adapters (each a Cordis plugin)

| Adapter | Package | Auth |
|---|---|---|
| DeepSeek official | `llm-deepseek` + `llm-deepseek-api-key` / `llm-deepseek-account` | API key / Account token |
| pi-ai catalogs | `llm-pi-ai` | Gateway routes |
| DeepSeek extensions | `deepseek-llm-api-extensions` | Lifecycle-owned metadata |

Each adapter implements the **provider-neutral `ModelStream` trait**:

```ts
// packages/llm/llm/src/index.ts (conceptual)
interface ModelStream {
  request(request: ModelRequest): ModelStream;
}
```

### BYOK — the user plugs in their own keys

`llm-deepseek-api-key` and `llm-deepseek-account` are **separate plugins** that provide authentication. The core `llm` service doesn't know about keys — it only knows "a provider is registered." Keys are injected via `inject` at plugin load time, and the adapter reads them from its scoped context.

**Key insight:** *The harness never stores keys.* The user's keyring (OS keychain, env var, config file) feeds the auth plugin, which injects a credential handle into the provider adapter's context. The adapter uses it per-request.

### Provider routing (`packages/core/agent/src/model-selection.ts`)

`AgentModel` is the durable selection surface:

```ts
// AgentModel.select({ provider, model, reasoningEffort? })
// AgentModel.current() → { provider, model, reasoningEffort } | undefined
```

- `select()` assigns the complete choice for the next unprepared step
- Omitted `reasoningEffort` uses adapter defaults (does not inherit)
- Cross-provider switch emits a **model-switch notice**: `[model changed: assistant turns above this point were generated by <previous>; the session continues with <next>]`
- Labels include providers only for cross-provider switches

### Rust target — `mch-dsh::llm`

| Piece | TypeScript | Rust |
|---|---|---|
| ModelStream trait | `ModelStream` | `trait ModelStream { async fn request(&self, req: ModelRequest) -> ModelStream }` |
| Provider adapter | `Plugin` + `ctx.llm` | `struct ProviderAdapter { name: String, stream: Box<dyn ModelStream> }` |
| BYOK auth | Separate plugin + `inject` | `trait CredentialProvider { fn get_credential(&self) -> Credential }` |
| Model selection | `AgentModel.select()` | `struct ModelSelector { current: Option<ModelSelection>, select: fn(Selection) }` |
| Model-switch notice | Emitted in loop | `fn emit_switch_notice(prev: Selection, next: Selection) -> String` |

---

## 3. Super-Agent Routing — `ctx.agents` + `agent.model`

The `packages/core/agent/` package is the **agent registry and handle surface**. It does **not** create model requests — that's the driver (`dsh-agent-loop`).

### Agent handle (`src/index.ts`)

```ts
// Create
const handle = await ctx.agents.create({
  sessionId,
  agentOptions: { provider: 'deepseek', model: 'deepseek-chat' },
})

// Drive
handle.agent.followup({ content, source })
handle.agent.steer({ content, source })
handle.agent.inject({ content })  // model-facing context, no wake
await handle.agent.whenIdle()

// Scope
handle.agent.ctx  // registrations here are agent-scoped, unwind on dispose()

// Dispose
await handle.dispose()  // stops loop, unregisters, removes session, unwinds scope
```

### Initiator scope — process-local attribution

`ctx.agents.withInitiator(agent, ...)` wraps the driver's lifetime. `AsyncLocalStorage` chain carries the exact live `Agent` through async chains. `withoutInitiator()` hides it for unrelated work. **Boundary is process-local** — workers, child processes, HTTP, durable queues, restarts must materialize identity explicitly.

### Model selection durability

`AgentModel` (src/model-selection.ts) binds the Session, live options, and a projection reader. The loop captures a **detached selection** before step preparation and retains it across retries. A later concurrent `select()` waits for another step.

### Scoped registrations = capability isolation

`Agent.ctx` is the agent's scoped context. Registrations made through it (tools, prompt sections, variables, event listeners, restrictions) apply to that agent alone and unwind on disposal. This is how agent presets give one session a different capability set without affecting neighbors.

### Rust target — `mch-dsh::agent`

| Piece | TypeScript | Rust |
|---|---|---|
| Agent registry | `AgentRegistry` | `struct AgentRegistry { agents: HashMap<Id, AgentHandle> }` |
| Agent handle | `AgentHandle` | `struct AgentHandle { id: Id, session: Arc<Session>, ... }` |
| Scoped context | `Agent.ctx` | `struct ScopedContext { parent: Arc<Context>, registrations: RegistrationSet }` |
| Model selection | `AgentModel` + `select()` | `struct ModelSelector { current: Option<Selection>, select: fn(Selection) }` |
| Initiator scope | `AsyncLocalStorage` | `thread_local! { static INITIATOR: RefCell<Option<AgentId>> }` |
| Agent events | `agent/*` events | `enum AgentEvent { PreStep, RequestError, TurnStopping, ... }` |

---

## 4. Dynamic Package Runtime — Browser/Host Halves

The `packages/extensions/cordis-client-runner/` (browser) + `cordis-host-runner/` (host) implement **dynamic package loading** — Cordis packages loaded at runtime, not build time.

### Architecture

- **Host runner** (`cordis-host-runner`): registry, run orchestration, slot management
- **Client runner** (`cordis-client-runner`): browser half, guard-wrapped eval, loader mount
- **Protocol**: `cordis/request-run` event → host half loads → source fetch → browser half eval → resolution

### Lifecycle

1. Host receives `cordis/request-run` (definition + revision)
2. Host half loads first (short-circuits on failure)
3. Source fetch from registry
4. Browser half: async function eval with symbol surface (`React`, `console`, `styles`, `host`)
5. Guard-wrapped plugin mounted through `loader.create`
6. Resolution: loaded revision or failing stage

### Hot-reloadable

- Newer revision replaces loaded one
- Refused resolution (stale revision) keeps what's loaded
- Host retraction retracts stale load
- Page refresh starts clean — host holds definition, page doesn't run until asked

### Rust target — `mch-dsh::dynamic`

| Piece | TypeScript | Rust |
|---|---|---|
| Host runner | `cordis-host-runner` | `struct HostRunner { registry: DynamicRegistry, runner: Orchestrator }` |
| Client runner | `cordis-client-runner` | `struct ClientRunner { guard: Guard, evaluator: Evaluator, loader: Loader }` |
| Source eval | `eval` in browser | `wasmtime::Engine` + `Module` for sandboxed WASM |
| Guard | Whitelist façade | `struct Guard { allowed: HashSet<ServiceName> }` |
| Source fetch | HTTP from registry | `async fn fetch_source(id: &str) -> Source` |

---

## 5. What I take, and what I leave

**Take:**
1. **Cordis fiber lifecycle** — the `PENDING`→`LOADING`→`ACTIVE`→`UNLOADING`→`DISPOSED` state machine with effect cleanup is the correct model for plugin lifecycle
2. **BYOK as separate plugin** — auth is a capability, not a core concern; inject credentials at load time
3. **Provider-neutral `ctx.llm`** — one service, many adapters, user brings keys
4. **Durable model selection** — `AgentModel.select()` + detached selection + model-switch notices
5. **Agent-scoped context** — `Agent.ctx` registrations unwind on disposal = capability isolation
6. **Initiator scope** — process-local `AsyncLocalStorage` for attribution
7. **Dynamic package runtime** — Cordis packages loaded at runtime, browser/host split, HMR
8. **Plugin manifest schema** — `Plugin.Base` with `Config` (standard-schema), `inject`, `provide`, `intercept`
9. **Hot-reloadable plugins** — `@cordisjs/plugin-hmr` pattern
10. **Tool executors as plugins** — PTC mode + native, scoped selection

**Leave:**
1. **TypeScript-specific patterns** — decorators (`@Inject`), `AsyncLocalStorage`, dynamic `eval`, prototype-based `inject` resolution
2. **Cordis-specific event waterfall** — `internal/config` → `internal/update` → `internal/commit` is Cordis-specific; McHarness wants simpler config
3. **TypeScript-specific SDK generation** — `run_code` with generated SDK bindings is TS-specific
4. **`@standard-schema/spec`** — Rust has `schemars`/`validator`/`valico`; different ecosystem
5. **Waterfall/dispatch pattern** — Cordis's `Context.waterfall()` is powerful but complex; McHarness uses simpler hook chains
6. **Browser/host split** — McHarness is native-first; dynamic packages are a future capability
7. **Cosmokit utilities** — `@deepseek-ai/cosmokit` is a TS toolkit; Rust has `futures`, `tokio`, `tracing`
8. **Volatile config references** — Cordis's `Volatile<T>` with `.get()` is a TS pattern; Rust uses `Arc<RwLock>` or `watch` channels

---

## 6. File map

| Capability | File | Lines (approx) | McHarness crate |
|---|---|---|---|
| Cordis context | `vendor/cordis/src/context.ts` | 200 | `mch-dsh::cordis` |
| Cordis fiber | `vendor/cordis/src/fiber.ts` | 820 | `mch-dsh::cordis` |
| Cordis registry | `vendor/cordis/src/registry.ts` | 337 | `mch-dsh::cordis` |
| Cordis events | `vendor/cordis/src/events.ts` | 250 | `mch-dsh::cordis` |
| Cordis reflect | `vendor/cordis/src/reflect.ts` | 200 | `mch-dsh::cordis` |
| Cordis fiber states | `vendor/cordis/src/fiber.ts:147` | 8 | `mch-dsh::cordis` |
| LLM core | `packages/llm/llm/` | ~500 | `mch-dsh::llm` |
| DeepSeek adapter | `packages/llm/llm-deepseek/` | ~300 | `mch-dsh::llm::deepseek` |
| pi-ai adapter | `packages/llm/llm-pi-ai/` | ~200 | `mch-dsh::llm::pi_ai` |
| BYOK auth | `packages/llm/llm-deepseek-api-key/` | ~150 | `mch-dsh::llm::auth` |
| Model selection | `packages/core/agent/src/model-selection.ts` | 400 | `mch-dsh::agent` |
| Agent registry | `packages/core/agent/src/index.ts` | 600 | `mch-dsh::agent` |
| Agent events | `packages/core/agent/src/dispatch.ts` | 300 | `mch-dsh::agent` |
| Cordis client runner | `packages/extensions/cordis-client-runner/` | 500 | `mch-dsh::dynamic` |
| Cordis host runner | `packages/extensions/cordis-host-runner/` | 400 | `mch-dsh::dynamic` |
| MCP client | `packages/mcp/mcp-client/` | 300 | `mch-mcp` (exists) |
| MCP manager | `packages/mcp/mcp-manager/` | 400 | `mch-mcp` (exists) |
| Skill registry | `packages/skill/skill/` | 300 | `mch-dsh::skill` |
| Tool executor PTC | `packages/core/tool-executor-ptc/` | 200 | `mch-dsh::tool::ptc` |

---

## 7. What I take, and what I leave (summary)

**Take:** The *architecture* — plugin kernel, BYOK, super-agent routing, dynamic runtime. These are language-agnostic patterns.

**Leave:** The *TypeScript implementation* — decorators, AsyncLocalStorage, eval, waterfall, standard-schema, cosmokit. These are TS-specific.

The Forge target is `mch-dsh` with sub-crates:
- `mch-dsh-cordis` — plugin kernel (fiber, registry, effects)
- `mch-dsh-llm` — provider-neutral LLM + BYOK
- `mch-dsh-agent` — agent registry, handle, model selection
- `mch-dsh-dynamic` — dynamic package runtime (future)
- `mch-dsh-skill` — skill registry (future)
- `mch-dsh-tool` — tool executors (future)

The Forge Ledger entry for DSH is `extracting` → `forged`. Crown jewels located. Ready to forge.

---

## 8. Next target queue

Per `forge/forge-timeline.md` Phase 8:

1. **DSH** — ✅ Dossier complete, ready for `mch-dsh` crate
2. **REA (morluto/rea)** — behavioral analysis + executable inspection
3. **Cloudflare security-audit-skill** — multi-stage audit pipeline
4. **Hexstrike** — offensive tooling orchestration (scoped)
5. **Bug-Bounty Agents** — fail-closed scope authority
6. **OpenWorker** — task execution and delivery

Next order: **REA autopsy** → `forge/rea` → `docs/ingestor/rea.md` → `mch-rea` crate.

---

*Forge Ledger entry: `DSH — extracting → forged`*  
*Lineage recorded. Crown jewels located. Ready to forge.*