# HexStrike AI (0x4m4/hexstrike-ai) — Source-Level Dossier

> Lineage: Security Team (scope-safety bound) — offensive tooling orchestration. Audit of local clone, verified against source. Authorized-targets context only.

## 1. REPO MAP

HexStrike is a deliberately two-process architecture, stated in the header comment of `hexstrike_server.py` ("Architecture: Two-script system"):

- **`hexstrike_server.py`** — a monolithic **Flask** HTTP server (not FastAPI; `app = Flask(__name__)` at line 94) binding to `HEXSTRIKE_HOST:HEXSTRIKE_PORT` (default `127.0.0.1:8888`, lines 98–99). It owns every subprocess. Entry: `app.run(host="0.0.0.0", port=API_PORT, debug=DEBUG_MODE)` at line 17289. Note: it listens on 0.0.0.0 despite the default env value, with **no authentication at all**.
- **`hexstrike_mcp.py`** — a **FastMCP** stdio bridge. `main()` (line 5426) builds a `HexStrikeClient` (HTTP client, `requests.Session`) pointed at the server via `--server http://IP:8888`, then `setup_mcp_server()` returns a `FastMCP("hexstrike-ai-mcp")` and calls `mcp.run()`.
- **`hexstrike-ai-mcp.json`** — the Claude Desktop-style config: `command: python3, args: [hexstrike_mcp.py, --server, http://IP:8888]`, `timeout: 300`, and `"alwaysAllow": []` with the comment *"Turn off alwaysAllow if you don't want autonomous execution!"* — the only "scope-safety" mechanism in the repo is a client-side config knob.

Deployment model: the MCP process runs on the AI host (stdio to the agent); the Flask server runs on the box where the 150+ binaries live. This split is the transferable idea: **thin MCP shim + fat local executor**.

## 2. MCP BRIDGE

- **Registration pattern**: 151 `@mcp.tool()` decorated closures nested inside `setup_mcp_server()` (`hexstrike_mcp.py:267`), closing over `hexstrike_client`. Each tool is a plain typed Python function — FastMCP derives the JSON schema from Python type hints and the docstring. Example: `nmap_scan(target: str, scan_type: str = "-sV", ports: str = "", additional_args: str = "")` (line 284).
- **Uniform tool body**: build a dict of params → `hexstrike_client.safe_post("api/tools/<tool>", data)` with `data["use_recovery"] = True` → log success/failure with colors → surface `recovery_info` and `alternative_tool_suggested` from the response to the agent. That's it — every tool is a ~30-line proxy.
- **`HexStrikeClient`** (line 147): `safe_get`/`safe_post` wrap `requests` with `DEFAULT_REQUEST_TIMEOUT = 300` and return `{"error": ..., "success": False}` dicts on failure rather than raising — so the agent always receives structured JSON, never a stack trace. `execute_command()` (line 245) posts to the generic `/api/command`.
- **Schemas are manually scoped per tool**: each tool exposes a *narrow, named* parameter set (`severity`, `tags`, `wordlist`…) plus a single `additional_args: str` escape hatch that gets spliced into the shell command server-side. This is the bridge's key design trade-off: friendly typed surface for the LLM, raw shell for the operator.
- Line 3836: a meta-tool `get_telemetry()` exposes server telemetry back to the agent; there's also a health/status tool (~line 3795) reporting `tools_status` — availability introspection as an MCP tool.

## 3. SERVER ARCHITECTURE

- **Endpoints** (all `@app.route`): `/health`; generic `/api/command` (POST, arbitrary shell — line 9137); file ops `/api/files/{create,modify,delete,list}`; `/api/payloads/generate`; telemetry/cache/process endpoints (`/api/processes/{list,status,terminate,pause,resume}`, lines 9291–9392); **~150 per-tool endpoints** `/api/tools/nmap`, `/api/tools/gobuster` (10377), `/api/tools/nuclei` (10429), `/api/tools/metasploit` (11049), etc.; async machinery `/api/process/execute-async` (16694), `/api/process/get-task-result/<task_id>` (16721), pool stats/scaling/health endpoints (16742–16979).
- **Command construction**: each tool endpoint does string interpolation into a shell line — e.g. `nmap()` builds `f"nmap {scan_type} -p {ports} {additional_args} {target}"` (lines 10344–10352). Validation is minimal: `nmap` checks only that `target` is non-empty; `gobuster` validates `mode` against an enum list (10395); `nuclei` checks `target`. **No shell escaping anywhere.**
- **Execution core**: `EnhancedCommandExecutor` (line 6783) wraps `subprocess.Popen(command, shell=True, ...)` with reader threads for stdout/stderr (`_read_stdout`/`_read_stderr`), a progress-spinner thread (`_show_progress`), `process.wait(timeout=COMMAND_TIMEOUT)` where `COMMAND_TIMEOUT = 300` (line 6664), then graceful `terminate()` → 5s wait → `kill()`. Notably, **timeout with partial output counts as success** (line 6946: `success = True if self.timed_out and (self.stdout_data or self.stderr_data)...`) and the result includes `"partial_results": True` — a good pattern for long scans.
- `execute_command(command, use_cache)` (8636) wraps the executor with `AdvancedCache` (LRU+TTL, class at 5085); `execute_command_with_recovery()` (8664) layers the retry loop (below).

## 4. TOOL CATEGORIES

The inventory is organized by **comment-banner sections** (grep-verified section headers), which matter because both files mirror the same taxonomy and the agent reads the docstrings:

- Core network scanning (`nmap_scan`, line 283), Cloud security (prowler/trivy/scout-suite/cloudmapper/pacu/kube-hunter/falco/checkov/terrascan, ~lines 417–10957 server-side), File ops & payload generation, Python environment management, "additional security tools", enhanced network pentest (1395), binary analysis & RE (1795), binary exploitation (2007), web app security (2370), AI payload generation (2799), advanced API testing (2936), vuln intelligence (`monitor_cve_feeds`, `generate_exploit_from_cve`, `discover_attack_chains`, `research_zero_day_opportunities` — lines 4012–4151), CTF, generic `execute_command` (3969).
- Server-side there are also *semantic workflow managers*: `BugBountyWorkflowManager` (2447) with `BugBountyTarget(scope=..., out_of_scope=...)` (2438) and impact-prioritized workflows; `CTFWorkflowManager` (2795); `IntelligentDecisionEngine` (572); `AIExploitGenerator` (7027, templated exploit source generation). Most of these emit *plan JSON* — the actual execution loop is left to the LLM, which orchestrates tool-by-tool.

## 5. EXECUTION & SAFETY MODEL

What exists:
- **Timeouts**: 300s default (`COMMAND_TIMEOUT`, line 6664), graceful-terminate→kill ladder; MCP client also has a 300s request timeout.
- **Process registry + kill**: `EnhancedProcessManager.process_registry` (5215) keyed by PID with an `RLock`; `terminate_process_gracefully(pid, timeout=30)` (5340).
- **Resource governance**: `ResourceMonitor` (5423, psutil), auto-*nice* — if CPU > 85%, the command is prefixed with `nice -n 10` (line 5266–5269); auto-scaling thread pool (below); a health-score endpoint.
- **Partial-results-on-timeout** and result caching (TTL 30 min default).
- **Error classification & recovery**: `IntelligentErrorHandler` (1606) regex-classifies stderr into `ErrorType` (1558) and picks a `RecoveryAction` (1572): retry-with-backoff, retry-with-reduced-scope, switch tool, adjust parameters, graceful degradation, abort, or **`ESCALATE_TO_HUMAN`** (8777) — the only human-in-the-loop hook, and it's purely for *errors*, not for dangerous actions.

What is missing (verified absent):
- **No target validation**: no allowlist/denylist of IPs/domains/CIDRs anywhere on the tool paths (the only "scope" hit is the HTTP proxy framework's `_in_scope` at 13373, used only by that internal Burp-like class, not the tool endpoints).
- **No shell escaping / injection protection**: `additional_args` and `target` are interpolated into `shell=True` commands verbatim.
- **No authentication on the Flask API**, bound to 0.0.0.0 (17289) — anything on the network can POST `/api/command`.
- **No output size truncation** beyond `FileOperationsManager.max_file_size` (100MB, 8934); stdout/stderr accumulate unbounded in memory.
- **No concurrent-job cap on the sync path**, no per-tool rate limiting, no audit ledger beyond log lines and `telemetry.record_execution`.

## 6. BACKGROUND JOBS

Two-tier machinery:
- **Sync path** (most tools): request → `EnhancedCommandExecutor.execute()` blocks up to 300s → JSON response. Simple, and honestly the right default for scan-length commands.
- **Async path**: `POST /api/process/execute-async` (16694) → `EnhancedProcessManager.execute_command_async()` (5236) mints `task_id = f"cmd_{int(time.time()*1000)}_{hash(command) % 10000}"` (line 5238) and pushes onto a custom `ProcessPool` (4877): a `queue.Queue` plus resizable daemon-worker threads (`_worker_thread`, 4932), state dict `active_tasks` (queued→running) and `results` (completed/failed with timing and worker id). Poll via `GET /api/process/get-task-result/<task_id>` (16721), which returns `{"status": ..., "result": ...}` or 404 on `not_found`. `ProcessPool._monitor_performance` (5007) scales workers between min/max on load ratio. Note the flaws to avoid: unbounded `results` dict (memory leak), hash-collidable task IDs, and thread-based "process pool".

## 7. CROWN JEWELS

1. **Thin MCP shim over a local executor** — `hexstrike_mcp.py::setup_mcp_server` + `HexStrikeClient.safe_post`. Keeps agent-facing schema stable while the executor evolves independently. *Rust:* a `mcp-bridge` crate whose tools are thin structs over an internal executor gRPC/HTTP API.
2. **Docstring-derived typed tool surface with a single `additional_args` escape hatch** (`nmap_scan`, mcp:284). LLM-friendly typed params, operator-grade raw args. *Rust:* serde-based per-tool arg structs + one `extra: Vec<String>` passthrough field.
3. **Structured-JSON-always error contract** (`safe_get`/`safe_post`, mcp:192–243) — failures become `{"success": false, "error": ...}` payloads, never panics into the agent loop. *Rust:* `Result<ToolOutput, ToolError>` flattened to JSON in the MCP handler.
4. **Partial results on timeout** (`EnhancedCommandExecutor.execute`, server:6925–6981) — kill at deadline but return captured stdout with `timed_out/partial_results` flags. Scans are exactly where this matters. *Rust:* `tokio::time::timeout` + incremental reader into a `Vec<u8>` buffer.
5. **Terminate-then-kill process ladder with a PID registry** (`terminate_process_gracefully`, server:5340; `EnhancedProcessManager`, 5208). *Rust:* PID → `Child` map behind `Mutex`, SIGTERM→wait(30s)→SIGKILL (ideally in a process group).
6. **Error classification → typed recovery strategy incl. ESCALATE_TO_HUMAN** (`execute_command_with_recovery`, server:8664, esp. 8777; `IntelligentErrorHandler`, 1606). The taxonomy (backoff / reduced scope / alternate tool / abort / escalate) is directly reusable. *Rust:* an enum `RecoveryStrategy` driving a bounded retry loop with history appended to the result.
7. **Task-id registry + polling result endpoint** (`ProcessPool.submit_task/get_task_result`, server:4904/4922; endpoints 16694/16721). *Rust:* `HashMap<TaskId, JobState>` behind a lock; MCP tool `get_job_result(id)` — but bound result retention and use opaque IDs.
8. **Availability introspection as a tool** (`/health` reporting `tools_status`; mcp health tool ~3795) — lets the agent plan around missing binaries. *Rust:* startup binary-discovery pass exposed as a `tool_capabilities()` MCP call.

## 8. WHAT NOT TO COPY

- **The 150-tool inventory itself**: 151 `@mcp.tool()` functions in one 5,471-line closure and ~150 near-identical Flask endpoints is unmaintainable duplication; McHarness should generate tools from a declarative tool-descriptor table (name, arg schema, command template, risk class).
- **String-interpolated shell construction** (`nmap()` 10344–10352) — injection-prone; use argv arrays.
- **`AIExploitGenerator`, `advanced_payload_generation(evasion_level="nation-state")`, `research_zero_day_opportunities`** (mcp:4044/4200/4117): exploit-template source generation is product LARP with heavy security liability, and it's mock-data-driven (`_generate_mock_vulnerabilities` around server:8600). McHarness's ledger-bound offensive scope doesn't want weaponized-payload generation plumbing.
- **The visual engine** (`ModernVisualEngine`, 105, ~800 lines of ANSI art) and the fake-progress spinner.
- Thread-based "process pools", unbounded caches/`results` dicts, and the `pickle` import path (unused surface worth noting).

## 9. SCOPE-SAFETY ASSESSMENT

**What exists:** almost nothing enforceable. (a) Client-side `alwaysAllow: []` in `hexstrike-ai-mcp.json` — a config convention, not a control; (b) `ESCALATE_TO_HUMAN` recovery action (server:8777) — error-path only; (c) `BugBountyTarget.in_scope/out_of_scope` fields (2438) — advisory metadata used to *plan*, never enforced; (d) `_in_scope()` (13373) — real host checks, but only inside one internal HTTP-proxy class; (e) narrow per-tool MCP schemas plus endpoint enums (e.g. gobuster `mode`, 10395) — parameter validation, not authorization. Meanwhile `execute_command` (mcp:3969 → server:9137) hands the agent an **unrestricted root-equivalent shell** with no auth on 0.0.0.0.

**What McHarness must add** (the actual lesson of this audit): a server-enforced **authorization scope object** bound to each job — target CIDR/domain allowlist with subdomain policy (borrow the `_in_scope` shape, 13373, but apply it at *every* tool invocation, not one class); argv-array execution instead of `shell=True` so injected targets can't break containment; a command allowlist generated from the tool-registry rather than a generic `execute_command`; per-job and global concurrency, wall-clock, and output-size caps; an immutable audit ledger of (job, scope, command, exit, output digest) to satisfy the "Security Team lineage (scope-safety bound)" entry; and a mandatory human-approval gate for high-risk tool classes — HexStrike's escalation hook proves the seam exists, it just never gets used for authorization decisions.
