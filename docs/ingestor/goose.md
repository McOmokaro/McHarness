# Goose Ingestor Dossier

**Lineage:** Goose (block/goose) — Apache-2.0, Rust core
**Status:** `extracting` → `forged`
**Forge target:** `mch-mcp` (Rust), wired into `mch-core` and `mch-loop`

---

## What I read, and how

I cloned the repo (`--depth 1`, ~2.5k files) into `forge/goose` and read source, not READMEs. Goose is a Rust agent, so unlike Hermes there is no translation tax — the structure transfers almost one-to-one. It builds its MCP layer on `rmcp`, the official Rust MCP SDK. That is the first thing worth deciding: **we re-implement the protocol, we do not vendor `rmcp`.** Goose gets correctness cheap by leaning on the SDK; McHarness wants the transport and the aggregation logic to be ours, so the seams are visible and the "Whole" law is not someone else's abstraction.

Three threads, all load-bearing:

- **Transports** — `crates/goose/src/agents/extension_manager/stdio.rs`, `streamable_http.rs`, `builtin.rs`
- **The client** — `crates/goose/src/agents/mcp_client.rs` (1442 lines)
- **Aggregation** — `crates/goose/src/agents/extension_manager/mod.rs` (2732 lines), `.../lease.rs`

Supporting cast: `crates/goose/src/agents/extension.rs` (the error taxonomy), `crates/goose-provider-types/src/mcp_utils.rs` (resource→text), `crates/goose/src/agents/state_machine/ops_toolcalling.rs` (dispatch + error surfacing).

Everything below is a claim I can point at a file for. Where I extrapolate, I say so.

---

## 1. Transports — how Goose spawns and connects

Goose has four `ExtensionConfig` variants, dispatched in `ExtensionManager::start` (`extension_manager/mod.rs:872`): `StreamableHttp`, `Stdio`, `Builtin`, `Platform`. Every one funnels into the same `McpClient::connect`, so from the client's point of view there is exactly one kind of peer.

### stdio — `stdio.rs`

The stdio path is 101 lines and every one earns its place.

- **Spawn**: `Command::new(resolve_command(cmd))` with args + env, then `spawn_long_lived_mcp_subprocess(command)`. The child's stdin/stdout become the JSON-RPC pipe.
- **Docker placement**: if the extension is placed in a container, it becomes `docker exec -i <id> <cmd> <args>` (`docker_exec`), and the same client connects over that pipe. The container is a placement detail, not a transport.
- **PATH recovery** (`crates/goose-mcp/src/subprocess.rs`): when launched from a desktop app, the process inherits a truncated PATH. Goose spawns a **login shell** (`$SHELL -l -i -c 'echo $PATH'`) in a new session, takes the last non-empty line, and prepends it to the child's PATH. Cached in a `OnceLock`. This is the fix for "works in my terminal, not in the app" and we will need the identical trick for the TUI.
- **stderr is the diagnosis** (`stdio.rs:88`): a task drains the child's stderr to a string. If `McpClient::connect` fails, that buffered stderr is wrapped into `ProcessExit` and returned as the error. This is why Goose can tell you *why* an MCP server died at startup instead of a bare "connection closed".

### streamable HTTP + SSE — `streamable_http.rs`

1148 lines, and most of them are the part everyone gets wrong: **auth**. The transport is `StreamableHttpClientTransport`; the crown jewels are around it.

- **OAuth fallback detection** (`is_oauth_auth_failure`, line 29): after a failed initialize, Goose inspects the transport error and treats a typed `AuthRequired`/`InsufficientScope`, a bare `HTTP 401`/`403` body, or a message containing "Auth required" as an OAuth trigger — then re-runs the connect through `oauth_flow_with_challenge`.
- **Seed discovery from the challenge** (`auth_challenge_from_error`, line 82): the `WWW-Authenticate` header off the 401/403 is extracted and fed to the OAuth flow, so discovery starts from what the server told us instead of probing well-known URLs. Reactive step-up auth for post-init 401s lives in `auth_challenge_from_service_error` (line 117).
- **Credential lifecycle**: a `GooseCredentialStore` persists tokens; on a refresh failure it **clears the credentials** so the next attempt restarts the flow rather than looping on a dead token.
- **Compat retry**: `connect_with_legacy_retry` retries once when the error contains `"empty sse stream"` — a documented workaround with a TODO to remove it when the SDK handles empty discovery SSE.
- **Unix socket transport** (`connect_over_unix_socket`, line 651) uses the same client over a local socket.

### builtin / platform — `builtin.rs`, `goose-mcp/src/lib.rs`

Built-in "MCP servers" (memory, tutorial, computer-controller, autovisualiser) are **in-process**, connected over `tokio::io::duplex(65536)` pipes: `extension_fn(server_read, server_write)` spawns the server task, and `McpClient::connect((client_read, client_write), ctx)` connects to it. Same client, same protocol, zero subprocess. If placed in Docker, the builtin falls back to `goose mcp <key>` as a child process. This is the pattern for McHarness' own native tools: **they speak MCP to themselves over a duplex, so there is no second code path for "internal" tools.**

---

## 2. Aggregation — one tool list from many servers

This is the heart of the dossier. Multiple MCP servers each expose tools; the model sees **one flat list**. Goose does it in three stages.

### Stage 1 — per-extension naming and provenance: `Extension::fetch_public_tools` (`extension_manager/mod.rs:341`)

For each extension, in a cursor loop over `list_tools`:

- Skip any tool the config's `available_tools` excludes.
- **Namespace the name**: `tool.name = format!("{}__{}", self.key, tool.name)` — unless the extension is first-class/unprefixed. Collisions across servers are impossible by construction.
- **Tag ownership**: inject `goose_extension = <key>` into the tool's `_meta`. That key is how a later call is routed back to the right server, and how the model's mangled name is recovered (below).
- **Normalize the input schema** (`normalize_input_schema`) — servers ship malformed JSON Schemas; this repairs them once, at the edge.
- Cache the result under `(scope_id, tools_version)`.

The version is the elegant part: `tools_version` is an `AtomicU64` bumped by the client's `on_tool_list_changed` handler (`mcp_client.rs:208`). When a server says its tool list changed, the cache for that extension is invalidated and rebuilt on the next read. No polling.

### Stage 2 — the catalog: `ToolCatalog::build` (`lease.rs:118`)

Across *all* extensions at once:

```rust
let lists = futures::future::join_all(
    extensions.iter().map(|e| e.public_tools(scope_id))
).await;
```

Concurrent listing, then merge into `entries: Vec<CatalogEntry>` + `by_name: HashMap<String, usize>`:

- **Dedup**: a name already in `by_name` is dropped with a warning — "keeping the earlier extension's". First writer wins, deterministically.
- **Provider-safe names** (`provider_safe_tool_name`, line 180): MCP allows names OpenAI rejects. If a name is longer than 64 chars or carries anything outside `[A-Za-z0-9_-]`, it is sanitized to `_`, truncated to 55, and suffixed with `_{blake3(name) first 8 hex}` — so `db.query` and `db_query`, or two names sharing a 64-char prefix, stay distinct.
- **Keep the server name**: each entry stores `server_name` (the name minus the `{key}__` prefix) so dispatch knows what to send upstream.

`ExtensionLease::tools()` (`lease.rs:269`) returns the flat `Vec<Tool>`; `tools_for(name)` / `tools_excluding(name)` give filtered views. The catalog is a `OnceCell`, built once per lease.

### Stage 3 — hand to the provider: `prepare_tools_for_provider` (`reply_parts.rs:213`)

Before the tools reach the model:

- App-only tools (`_meta.ui.visibility` without `"model"`) are dropped (`is_tool_visible_to_model`).
- The list is **sorted by name** — "Stable tool ordering is important for multi session prompt caching." (line 208). This one line is worth stealing verbatim: an unsorted tool list invalidates the provider's prompt cache every turn.
- If the model can't call tools natively (`toolshim`), the tool JSON is injected into the system prompt and the tools array is emptied.

### Recovering the mangled name — `recover_mangled_tool_name` (`extension_manager/mod.rs:527`)

Models rewrite `developer__shell` into `developer.shell` (or strip a `functions.` prefix). Goose reconstructs the real tool from the emitted string by matching against known `(name, owner)` pairs. We will hit this exact failure and want the same recovery map.

---

## 3. Errors and timeouts — the two things that decide whether MCP feels solid

### The taxonomy — `extension.rs`

```rust
pub enum ExtensionError {
    Client(ClientError),              // a failed client call to an MCP server
    ConfigError(String),
    SetupError(String),
    TaskJoinError(JoinError),
    IoError(std::io::Error),
    InitializeError(Box<ClientInitializeError>),
    ProcessExit(Box<ProcessExit>),    // stderr captured from a dead child
    StartFailed(String),
}
```

`ProcessExit` (`extension.rs:19`) carries the child's **stderr** and the underlying init error — the difference between "server failed" and "server failed because `uvx` is not on PATH". Startup is lazy and memoized: `ExtensionSlot::runtime` (`mod.rs:238`) is a `OnceCell<Runtime>` where `Runtime = Running | Declined | Failed(String)`. A failed extension is remembered as failed, not retried into a loop; a platform extension that the host can't provide **declines** rather than registering with zero tools.

### The timeout+cancel pattern — `mcp_client.rs:620`

This is the single best snippet in the repo. Every request races three arms:

```rust
tokio::select! {
    result = receiver => result.map_err(|_| ServiceError::TransportClosed)?,
    _ = tokio::time::sleep(timeout) => {
        send_cancel_message(&peer, request_id, Some("timed out".to_owned())).await?;
        Err(ServiceError::Timeout { timeout })
    }
    _ = cancel_token.cancelled() => {
        send_cancel_message(&peer, request_id, Some("operation cancelled".to_owned())).await?;
        Err(ServiceError::Cancelled { reason: None })
    }
}
```

Two things matter. First, on timeout or cancellation it **sends a `notifications/cancelled` back to the server** before returning — the server learns to stop working, instead of running to completion and dumping a late result. Second, the client mutex is held **only for the send** (`send_request_with_context`, line 583); the actual wait happens outside the lock, so concurrent tool calls overlap instead of serializing.

Concurrency safety around callbacks uses an RAII guard: `register_active_tool_call` returns an `ActiveToolCallGuard` (`mcp_client.rs:144`) whose `Drop` unregisters the call — so cancellation and dropped reply streams are covered exactly like normal completion. This is how Goose correlates a server-initiated elicitation back to the exact in-flight tool call.

### Timeout value

`resolve_timeout` (`extension_manager/mod.rs:123`): per-extension `timeout` if set, else `GOOSE_DEFAULT_EXTENSION_TIMEOUT`, else `DEFAULT_EXTENSION_TIMEOUT = 300` seconds (`config/extensions.rs:10`). One budget per extension, overridable — not a global.

### Surfacing a failure to the model — `ops_toolcalling.rs`

A tool call returns `Result<CallToolResult, ErrorData>`. A transport error is converted with `ToolCallResult::from(Err(error))` (line 454) and recorded into the conversation as a tool response, correlated by `request_id` (`ToolBatch::record`). The model sees:

- **tool-level error**: `CallToolResult::error(vec![ContentBlock::text(...)])` with `is_error = true`.
- **transport/timeout error**: the `Err` path, likewise surfaced as a failed result.

Classification is shared (`emit_post_tool_use`, line 217): `Ok(r) if r.is_error != Some(true)` → success; everything else → failure. Because the failure lands as a normal tool result, the model can read it and recover on the next turn — the error is data, not a crash.

`large_response_handler::process_tool_response` runs on every result before it is recorded, capping oversized outputs so one chatty server cannot blow the context.

---

## 4. Two more things worth taking

**Env hardening — `extension.rs:91`.** `Envs` rejects 31 keys at construction: `PATH`, `LD_PRELOAD`, `LD_LIBRARY_PATH`, `DYLD_INSERT_LIBRARIES`, `NODE_OPTIONS`, `PYTHONPATH`, `CLASSPATH`, `GOROOT`, `ComSpec`, and friends — the library-injection and interpreter-hijack set. A user-supplied `env` for an MCP server cannot smuggle a dynamic-linker preload. McHarness must ship this list; it is the difference between "bring your own env" and "bring your own RCE".

**Resource→text with injection defense — `mcp_utils.rs`.** `extract_text_from_resource` decodes text and blob resources, and every string path runs through `sanitize_unicode_tags`, which strips the Unicode "tag" block (U+E0000–U+E007F) — the Trojan-Source channel where a document reads one way to you and another to the agent. Binary blobs become `[Binary content (mime) - N bytes]`. This complements `mch-scope`: content from any MCP server is untrusted input.

---

## 5. Rust target — `mch-mcp`

We implement the protocol natively (no `rmcp` dependency), so the transport and aggregation are ours.

| Piece | Goose | `mch-mcp` |
|---|---|---|
| stdio transport | `extension_manager/stdio.rs` | `transport::StdioTransport` (spawn + newline-JSON-RPC, stderr→`ProcessExit`) |
| HTTP/SSE transport | `streamable_http.rs` | `transport::HttpTransport` (+ `auth::OAuthChallenge` from `WWW-Authenticate`) |
| In-process tools | `builtin.rs` (`tokio::io::duplex`) | `transport::DuplexTransport` — native tools speak MCP to themselves |
| Timeout + cancel | `await_response` (`mcp_client.rs:620`) | `McpClient::request_with` — `tokio::select!` + send `cancelled` on timeout |
| Send-lock discipline | `send_request_with_context` | hold the write lock only for the write |
| Protocol negotiation | `ClientLifecycleMode::{Auto,Discover}` | `Negotiation::Auto { preferred, legacy }` |
| Per-extension naming | `fetch_public_tools` | `catalog::namespace_tool` (`{key}__{tool}`) + `_meta` owner |
| Provider-safe names | `provider_safe_tool_name` (`lease.rs:180`) | `catalog::provider_safe_name` (64-char rule + hashed suffix) |
| Dedup across servers | `ToolCatalog::build` | `catalog::ToolCatalog::build` (`join_all` + first-wins) |
| Cache invalidation | `tools_version` + `on_tool_list_changed` | `tools_version: AtomicU64` |
| Schema repair | `normalize_input_schema` | `catalog::normalize_schema` |
| Name recovery | `recover_mangled_tool_name` | `catalog::recover_name` |
| Error taxonomy | `ExtensionError` (`extension.rs:40`) | `error::McpError` |
| Env hardening | `Envs::DISALLOWED_KEYS` | `env::DISALLOWED_KEYS` (31 keys) |
| Content sanitize | `sanitize_unicode_tags` | `content::sanitize_unicode_tags` |
| Default timeout | `DEFAULT_EXTENSION_TIMEOUT = 300` | `const DEFAULT_TIMEOUT_SECS: u64 = 300` |

---

## 6. What I take, and what I leave

**Take:**
1. The three-arm `tokio::select!` timeout — and the `cancelled` notification it sends upstream.
2. Namespacing every tool `{extension}__{tool}` plus an owner tag in `_meta`, with first-wins dedup across servers.
3. `provider_safe_tool_name`: the 64-char rule with a hash suffix that survives truncation.
4. Sorting the tool list for prompt-cache stability — one line, whole-session cache.
5. Lazy, memoized startup with a `Declined` state, so a missing platform service never becomes a zero-tool ghost.
6. `ProcessExit` carrying child stderr — the difference between a usable error and "connection closed".
7. In-process extensions over a duplex pipe: one client, one protocol, no "internal tool" special case.
8. The 31-key env denylist and the Unicode-tag stripper. Both are security, not polish.
9. OAuth seeded from the server's `WWW-Authenticate` challenge, with credential clearing on refresh failure.

**Leave:**
1. `rmcp` itself. We implement the wire protocol; depending on the SDK would hide the seams the "Whole" law wants visible.
2. The `compatible`/legacy SSE retry and its TODO — we target one protocol version and fail loudly on the rest.
3. The four `ExtensionConfig` variants as a public enum. McHarness collapses to `stdio | http | in_process`, since `Builtin` and `Platform` are both just in-process servers.
4. `toolshim` (tools-in-system-prompt for non-tool models). Every model McHarness routes to calls tools natively.
5. The Docker-exec placement path for now — it is a distribution concern, not an MCP one, and it can land later behind the same `Placement` indirection.

The aggregation layer is where the value is. Any harness can connect to one MCP server; the thing that makes a harness *whole* is that ten servers collapse into one clean tool list, with provenance intact, names that a provider will accept, a cache that invalidates itself, and errors that come back as data the model can act on. That is what `mch-mcp` is for.
