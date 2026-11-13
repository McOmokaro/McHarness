# HexStrike Ingestor Dossier

**Lineage:** HexStrike AI — `0x4m4/hexstrike-ai` (Muhammad Osama / 0x4m4), MIT License, Copyright (c) 2026. Pure Python, v6.0, commit `d689933` (2026-08-03). Cloned `--depth 1` into `forge/hexstrike-ai`.
**Status:** `extracting` → `forged`
**Forge target:** `mch-kali` (tool registry + tactical planner) and `mch-cyber` (hunt/swarm/MCP), aggregated through `mch-mcp`.

---

## What I read, and how

The whole platform is **two Python files** — no package tree, no plugin dirs:

- `hexstrike_mcp.py` — 5,470 lines, 217 KB. The MCP face.
- `hexstrike_server.py` — 17,289 lines, 734 KB. Everything else.
- `hexstrike-ai-mcp.json` — the 15-line client manifest (`python3 hexstrike_mcp.py --server http://IP:8888`, `timeout: 300`).
- `README.md` (779 lines) — claims "150+ security tools and 12+ autonomous AI agents".
- `requirements.txt` — the real dependency surface: `flask`, `fastmcp`, `requests`, `psutil`, `selenium`, `beautifulsoup4`, `aiohttp`, `mitmproxy`, `pwntools`, `angr`.

The README advertises "multi-agent". Read the code and it is **not** a multi-agent system — it is one decision engine, one error handler, and a pile of per-domain workflow managers, all driven by a single calling LLM over MCP. There is no intra-HexStrike agent loop. That distinction matters for the ingest: what we take is the **tool-selection brain and the tool registry**, not an agent architecture.

Everything below is pinned to a file and a line. Where I extrapolate, I say so.

---

## Architecture

HexStrike is a **two-process split**: an MCP server in front, an HTTP execution server behind it.

```
LLM  ──(MCP / stdio)──▶  hexstrike_mcp.py        ──(HTTP/JSON :8888)──▶  hexstrike_server.py  ──(subprocess)──▶  nmap, ffuf, …
        151 @mcp.tool()   FastMCP("hexstrike-ai-mcp")   HexStrikeClient.safe_post         Flask @app.route          shell tools
```

### Front: the MCP server — `hexstrike_mcp.py`

- **`setup_mcp_server(client)`** (`hexstrike_mcp.py:267`) builds `mcp = FastMCP("hexstrike-ai-mcp")` (line 277) and then defines **151 `@mcp.tool()` handlers inline in the function body** (one decorator = one MCP tool).
- **Transport is stdio.** `main()` (line ~5437) calls `mcp.run()` (line 5462) with FastMCP's default stdio transport; the LLM client launches the process per `hexstrike-ai-mcp.json`.
- **Schemas are implicit.** There is no hand-written JSON Schema anywhere. FastMCP derives each tool's `inputSchema` from the Python type hints of the decorated function. Example — `nmap_scan(target: str, scan_type: str = "-sV", ports: str = "", additional_args: str = "")` (line 284) becomes `{target, scan_type, ports, additional_args}` with `target` required. Defaults in the signature *are* the schema defaults. ~150 tool schemas are generated this way for free.
- **Every handler is the same shim.** Build a dict from the typed args, `hexstrike_client.safe_post("api/tools/<name>", data)`, log with ANSI colors (`HexStrikeColors`, line 31), return the JSON verbatim. The tool name maps to the endpoint by hand — `nmap_scan` → `api/tools/nmap`, `arjun_parameter_discovery` → `api/tools/arjun`. There is no name→endpoint rule; it is a literal string per tool.
- `data["use_recovery"] = True` is set on every scan tool so the server-side error handler runs by default.

### Back: the execution server — `hexstrike_server.py`

- **Flask app**, `app = Flask(__name__)` (line 94), `API_PORT = int(os.environ.get('HEXSTRIKE_PORT', 8888))` (line 98), run with `app.run(host="0.0.0.0", port=API_PORT)` at file end (line 17289 area). **150+ `@app.route(...)` handlers**, plus the tool-execution endpoints.
- **`HexStrikeClient`** (`hexstrike_mcp.py:147`): a `requests.Session`, `safe_get`/`safe_post` (line 192/220), connect-retry loop against `/health` with `MAX_RETRIES`, and a single `DEFAULT_REQUEST_TIMEOUT` (300 s per the manifest).
- **`execute_command`** (`hexstrike_server.py:8636`) is the single choke point every tool endpoint funnels through. It wraps subprocess execution with the cache (`HexStrikeCache`, line 6668), telemetry (`TelemetryCollector`, line 6736), and the async executor (`EnhancedCommandExecutor`, line 6783).
- **`execute_command_with_recovery(tool, command, params)`** (line 8664) is the wrapper the endpoints actually call when `use_recovery` is on — the seam where the intelligent error handler intervenes.
- The typical endpoint body (`/api/tools/nmap`, line 10327): parse `request.json`, **assemble a shell string** (`command = f"nmap {scan_type} -p {ports} {additional_args} {target}"`), call `execute_command_with_recovery`, `jsonify` the result. Flag construction is string concatenation, not structured args — a detail we will not copy.

### The tool registry is the `/health` endpoint — line 9023

There is no static tool registry object. The authoritative inventory is `health_check()` (line 9023): it lists every tool across **13 functional categories**, runs `which <tool>` for each via `execute_command`, and returns `tools_status{}`, `category_stats{}`, `total_tools_count`, `all_essential_tools_available`. Categories: `essential, network, web_security, vuln_scanning, password, binary, forensics, cloud, osint, exploitation, api, wireless, additional`. That endpoint *is* the machine-readable catalog.

### The 151 MCP tools, by family

Registered in `hexstrike_mcp.py` (families, not exhaustive): **network/scan** (`nmap_scan`, `nmap_advanced_scan`, `masscan_high_speed`, `rustscan_fast_scan`, `autorecon_comprehensive`, `arp_scan_discovery`, `nbtscan_netbios`, `rpcclient_enumeration`, `enum4linux_ng_advanced`), **web** (`gobuster_scan`, `ffuf_scan`, `feroxbuster_scan`, `dirsearch_scan`, `nikto_scan`, `wpscan_analyze`, `wfuzz_scan`, `xsser_scan`, `dotdotpwn_scan`, `burpsuite_scan`, `zap_scan`, `wafw00f_scan`), **crawl/params** (`katana_crawl`, `gau_discovery`, `waybackurls_discovery`, `hakrawler_crawl`, `arjun_parameter_discovery`, `paramspider_mining`, `x8_parameter_discovery`, `httpx_probe`, `qsreplace_parameter_replacement`, `uro_url_filtering`, `anew_data_processing`), **vuln** (`nuclei_scan`, `dalfox_xss_scan`, `jaeles_vulnerability_scan`, `sqlmap_scan`), **creds** (`hydra_attack`, `john_crack`, `hashcat_crack`, `hashpump_attack`, `responder_credential_harvest`), **cloud/container** (`prowler_scan`, `trivy_scan`, `scout_suite_assessment`, `cloudmapper_analysis`, `pacu_exploitation`, `kube_hunter_scan`, `kube_bench_cis`, `docker_bench_security_scan`, `clair_vulnerability_scan`, `falco_runtime_monitoring`, `checkov_iac_scan`, `terrascan_iac_scan`), **binary/pwn** (`ghidra_analysis`, `radare2_analyze`, `gdb_analyze`, `gdb_peda_debug`, `angr_symbolic_execution`, `pwntools_exploit`, `ropgadget_search`, `ropper_gadget_search`, `one_gadget_search`, `libc_database_lookup`, `pwninit_setup`, `checksec_analyze`, `binwalk_analyze`, `objdump_analyze`, `strings_extract`, `xxd_hexdump`, `volatility_analyze`, `volatility3_analyze`), **osint** (`amass_scan`, `subfinder_scan`, `fierce_scan`, `dnsenum_scan`), **exploit-gen** (`msfvenom_generate`, `metasploit_run`), **AI/intel** (`ai_generate_payload`, `ai_test_payload`, `ai_generate_attack_suite`, `monitor_cve_feeds`, `generate_exploit_from_cve`, `discover_attack_chains`, `correlate_threat_intelligence`, `research_zero_day_opportunities`, `threat_hunting_assistant`), **API** (`api_fuzzer`, `graphql_scanner`, `jwt_analyzer`, `api_schema_analyzer`, `comprehensive_api_audit`), **intelligence wrappers** (`analyze_target_intelligence`, `select_optimal_tools_ai`, `optimize_tool_parameters_ai`, `create_attack_chain_ai`, `intelligent_smart_scan`, `detect_technologies_ai`), **orchestration** (`bugbounty_*` ×8, `ai_reconnaissance_workflow`, `ai_vulnerability_assessment`), **ops/files** (`create_file`, `modify_file`, `delete_file`, `list_files`, `generate_payload`, `install_python_package`, `execute_python_script`, `execute_command`), **process control** (`list_active_processes`, `terminate_process`, `pause_process`, `resume_process`, `get_process_dashboard`), **visual/report** (`create_vulnerability_report`, `format_tool_output_visual`, `create_scan_summary`, `get_live_dashboard`), and **HTTP/browser** (`http_framework_test`, `http_repeater`, `http_intruder`, `browser_agent_inspect`).

---

## Crown Jewels

### 1. `IntelligentDecisionEngine` — the tool-selection brain (`hexstrike_server.py:572`)

This is the reason to ingest HexStrike. It is a **static, hand-tuned expert system** that maps a target to a scored, ordered tool plan — no learned model, just a curated knowledge base.

- **`analyze_target(target) -> TargetProfile`** (line 811). `TargetProfile` (line 474) carries `target_type`, `technologies`, `cms_type`, `open_ports`, `subdomains`, `ip_addresses`, `attack_surface_score`, `risk_level`, `confidence_score`.
  - `_determine_target_type` (line 837) is a regex/startsWith classifier → `TargetType` enum (line 445): `WEB_APPLICATION | API_ENDPOINT | NETWORK_HOST | CLOUD_SERVICE | BINARY_FILE | UNKNOWN`.
  - `_detect_technologies` (line 879) + `_detect_cms` (line 897) are **string heuristics** — `'wordpress' in target` → WordPress, `.php` → PHP. Explicitly a "simplified version" that never makes an HTTP request (comment at line 882). Honest, and worth noting: detection is a stub the config expects you to replace.
  - `_calculate_attack_surface` (line 907): type-weighted score, `min(score, 10.0)`. `_determine_risk_level` (line 933) buckets it into `critical/high/medium/low/minimal`. `_calculate_confidence` (line 945) starts at 0.5 and adds for each signal present.
- **`_initialize_tool_effectiveness()`** (line 581) — **the core asset.** A literal dict: `{target_type: {tool: effectiveness 0..1}}`. Example rows: `WEB_APPLICATION → {nuclei: 0.95, wpscan: 0.95, dalfox: 0.93, sqlmap: 0.9, arjun: 0.9, …}`; `NETWORK_HOST → {nmap-advanced: 0.97, nmap: 0.95, autorecon: 0.95, masscan: 0.92, rustscan: 0.9, …}`; `BINARY_FILE → {ghidra: 0.95, gdb-peda: 0.92, pwntools: 0.9, angr: 0.88, ropper: 0.88, …}`; `CLOUD_SERVICE → {prowler: 0.95, trivy: 0.9, kube-hunter: 0.9, checkov: 0.9, …}`; `API_ENDPOINT → {arjun: 0.95, x8: 0.92, httpx: 0.9, nuclei: 0.9, …}`. ~90 hand-scored tool/type pairs. This is a 1:1 replacement target for `mch-kali`'s hand-coded planner.
- **`select_optimal_tools(profile, objective="comprehensive")`** (line 971). Reads the effectiveness row for the profile's type, then filters by objective: `"quick"` → top-3 by score; `"comprehensive"` → all tools with score `> 0.7`; `"stealth"` → intersect with a fixed passive set `{"amass","subfinder","httpx","nuclei"}`. Then **appends technology pivots** — WordPress not yet selected → add `wpscan`; PHP → add `nikto`. Returns an ordered `List[str]`.
- **`_initialize_attack_patterns()`** (line 715) — named ordered playbooks (`web_reconnaissance`, `api_testing`, `network_discovery`, …): lists of `{tool, priority, params}`. `create_attack_chain(profile, objective)` (line 1462) turns a profile into an `AttackChain` (line 522) of `AttackStep`s (line 513).
- **`optimize_parameters(tool, profile, context)`** (line 1003) dispatches to **25+ hand-written `_optimize_<tool>_params` methods** (`_optimize_nmap_params`, `_optimize_gobuster_params`, `_optimize_nuclei_params`, `_optimize_sqlmap_params`, `_optimize_ffuf_params`, `_optimize_ghidra_params`, `_optimize_angr_params`, …). Each tunes per-tool flags from the profile. `ParameterOptimizer` (line 4635) and `optimize_parameters_advanced` (line 4702) are the extended path.
- **Exposure:** `/api/intelligence/{analyze-target, select-tools, optimize-parameters, create-attack-chain, smart-scan, technology-detection}` (routes at lines 9539–10097). **`intelligent_smart_scan`** (line 9672) is the flagship: `analyze_target` → `select_optimal_tools(...)[:max_tools]` → execute the chosen tools **in parallel** via a `tool_execution_map` dict of lambdas → `execute_nmap_scan`/`execute_gobuster_scan`/… (`hexstrike_server.py:9825`+), each itself calling `execute_command`, aggregating vulnerability counts.

### 2. `IntelligentErrorHandler` + `GracefulDegradation` — self-healing execution (`hexstrike_server.py:1606`, `:2201`)

- Taxonomy: `ErrorType`, `RecoveryAction`, `ErrorContext`, `RecoveryStrategy` (lines 1558–1605).
- **`_initialize_recovery_strategies()`** (line 1661): `ErrorType → [ordered RecoveryStrategy]` — e.g. permission error → retry with `sudo`; missing wordlist → substitute default; timeout → reduce thread count.
- **`_initialize_tool_alternatives()`** (line 1872): **tool → fallback tools** (e.g. `nmap` → `masscan, rustscan`). Exposed at `/api/error-handling/alternative-tools`. This is a clean, reusable "if tool X fails, try Y" map.
- `handle_tool_failure(tool, error, context)` (line 1983-2069) returns the chosen strategy; `FailureRecoverySystem` (line 4449) is a second, string-parsing variant of the same idea; `execute_command_with_recovery` (line 8664) is where it all hooks in. **Every tool call in the platform is error-handled by default.**

### 3. Per-domain workflow managers (the "12 agents" the README overcounts)

- **`BugBountyWorkflowManager`** (line 2447): `create_reconnaissance_workflow`, `create_vulnerability_hunting_workflow`, `create_business_logic_testing_workflow`, `create_osint_workflow`, with `_get_test_scenarios(vuln_type)`. Exposed via `/api/bugbounty/*` (routes 10098–10326). `FileUploadTestingFramework` (line 2699) backs `/api/bugbounty/file-upload-testing`.
- **CTF stack:** `CTFChallenge` (line 2784) carries a **`category`** field (`web, crypto, pwn, forensics, rev, misc, osint`); `CTFWorkflowManager` (line 2795) builds per-category workflows (`_create_workflow_steps`, `_generate_fallback_strategies`, `_identify_parallel_tasks`, `_create_validation_steps`); `CTFToolManager` (line 3492); `CTFChallengeAutomator` (line 3855); `CTFTeamCoordinator` (line 4072).
- **`CTFToolManager.tool_categories`** (line 3673) — **a second, orthogonal taxonomy of 28 categories** (`web_recon`, `web_vuln`, `web_discovery`, `web_params`, `crypto_hash`, `crypto_cipher`, `crypto_rsa`, `crypto_modern`, `pwn_analysis`, `pwn_exploit`, `pwn_debug`, `pwn_advanced`, `forensics_file`, `forensics_image`, `forensics_memory`, `forensics_network`, `rev_static`, `rev_dynamic`, `rev_unpack`, `osint_social`, `osint_domain`, `osint_search`, `misc_encoding`, `misc_compression`, `misc_esoteric`), each a list of tool names. Consumed by `get_category_tools` (line 3734) and `suggest_tools_for_challenge` (line 3741).
- Supporting detectors: `TechnologyDetector` (line 4223), `RateLimitDetector` (line 4344), `ParameterOptimizer` (line 4635).

### 4. Threat-intel and exploit generation (the genuine "AI")

- **`CVEIntelligenceManager`** (line 5750): `fetch_latest_cves(hours, severity_filter)` (NVD feed), `analyze_cve_exploitability(cve_id)`, `search_existing_exploits(cve_id)`. Backs `/api/cve/*`.
- **`VulnerabilityCorrelator`** (line 8510): `find_attack_chains(target_software, max_depth=3)` → correlated chains, `_generate_chain_recommendations`.
- **`AIExploitGenerator`** (line 7027): `generate_exploit_from_cve(cve_data, target_info)` → `_classify_vulnerability` → `_select_template` → `_generate_exploit_parameters` → `_apply_evasion_techniques` → `_advanced_obfuscation`. Emits from a family of typed exploit templates: `SQLiExploit` (7393), `XSSExploit` (7522), `FileReadExploit` (7642), `GenericExploit` (7787), `RCEExploit` (7965), `XXEExploit` (8091), `DeserializationExploit` (8181), `AuthBypassExploit` (8264), `BufferOverflowExploit` (8380). Note: templating + heuristics, **not** an LLM call.
- **`AIPayloadGenerator`** (line 14540): `generate_contextual_payload(target_info)` → `_get_payloads` → `_enhance_with_context` → `_generate_test_cases` → `_assess_risk_level`. Same template-driven pattern.
- **`AIPayloadGenerator`/`AIPayloadGenerator` and `AIExploitGenerator` are named "AI" but call no model** — they are curated payload/exploit libraries with a classifier in front. Treat them as data, not inference.

### 5. Execution infrastructure (worth mirroring, not rewriting)

- **`ProcessPool`** (line 4877): adaptive thread pool with `_scale_up`/`_scale_down` on load. **`AdvancedCache`** (line 5085): LRU + TTL, `_evict_lru`, `_cleanup_expired`. **`EnhancedProcessManager`** (line 5208): async command execution + system-resource auto-scaling (`_auto_scale_based_on_resources`) + `terminate_process_gracefully`. **`ProcessManager`** (line 5576). **`PythonEnvironmentManager`** (line 5705): creates per-task venvs. **`BrowserAgent`** (line 13623): Selenium-driven. **`HTTPTestingFramework`** (line 13281): mitmproxy-driven repeater/intruder.

---

## Rust Re-implementation Targets

We do **not** port the two-tier Python proxy, the string-concatenated shell commands, or the "AI" template libraries. We take the **catalog, the selection matrix, and the error/fallback logic** and land them in `mch-kali` (registry + planner) and `mch-cyber` (workflow/swarm), surfaced as MCP through `mch-mcp`.

### `mch-kali` — tool registry (`crates/mch-kali/src/tools.rs`)

Today `KaliTool` (tools.rs:16) enumerates only **12** tools (`Nmap, Ffuf, Sqlmap, Nuclei, Subfinder, Whatweb, Wpscan, Gobuster, Nikto, Hydra, Httpx, Amass`), each with `binary()`, `apt_package()`, and a `category() -> ToolCategory` (tools.rs:69). `ToolCategory` (tools.rs:128) has four variants: `Fingerprint, Recon, Fuzzing, Exploitation`. `ToolRegistry` (tools.rs:157) does `probe_all`/`installed`/`missing` via the `LazyLoader`.

| HexStrike | Location | `mch-kali` target |
|---|---|---|
| `/health` tool inventory, 13 functional categories | `hexstrike_server.py:9023` | grow `KaliTool` toward the full universe; make `ToolCategory` the 13-functional axis; add a `ToolManifest { name, category, apt_package, endpoint }` table on `ToolRegistry` |
| CTF `tool_categories`, 28 categories | `hexstrike_server.py:3673` | a second axis on the registry: `ToolCategory::Ctf(sub)`, or a `category_tags: &[&str]` so a tool can be both `Fuzzing` and `web_discovery` |
| `/health` `category_stats` availability rollup | `hexstrike_server.py:9100` | `ToolRegistry::availability_report()` — installed/missing per category (partly there: `installed()`/`missing()`) |

### `mch-kali` — the tactical planner (the biggest win)

Today the planner is **hand-coded**: `TacticalPlanner::derive_actions` (`crates/mch-kali/src/tactical.rs:277`) pushes fixed `PlannedAction`s (a hardcoded `Whatweb` probe, an `if fp.has(TechStack::WordPress)` → `Wpscan` + `Ffuf` pivot). HexStrike's `IntelligentDecisionEngine` is strictly more general and is the model to absorb.

| HexStrike | Location | `mch-kali` target |
|---|---|---|
| `_initialize_tool_effectiveness` — `{target_type → {tool → score}}` | `hexstrike_server.py:581` | a `ToolEffectiveness` matrix: `fn score(&self, TargetType/KaliTool) -> f32`, loaded as data, replacing the per-tech `if`-ladders in `derive_actions` |
| `TargetType` classifier (regex/prefix) | `hexstrike_server.py:445, 837` | new `enum TargetType` alongside `TechStack`, added to `TargetFingerprint` (`tactical.rs:84`) |
| `select_optimal_tools(profile, objective)` | `hexstrike_server.py:971` | `TacticalPlannerConfig` (`tactical.rs:151`) grows `objective: {Quick, Comprehensive, Stealth}`; the `>0.7` cutoff and passive-set intersection become the selection filter |
| `_initialize_attack_patterns` — ordered playbooks | `hexstrike_server.py:715` | seed data for a `TacticalPlan` template registry (`TacticalPlan`, `tactical.rs:115`) |
| `create_attack_chain` → `AttackChain`/`AttackStep` | `hexstrike_server.py:522, 1462` | `TacticalPlan { target, fingerprint, actions: Vec<PlannedAction> }` already *is* this; keep `PlannedAction { score, rationale }` |
| `optimize_parameters` + 25 `_optimize_<tool>_params` | `hexstrike_server.py:1003`+ | extend the per-tool `args()` builders (`tools.rs:257`+) with a `ParameterOptimizer` keyed on the fingerprint |
| `IntelligentErrorHandler` tool → fallback map | `hexstrike_server.py:1872` | `ToolRegistry::alternatives(tool) -> &[KaliTool]`; drive it from `bridge::execute_plan` (`crates/mch-kali/src/bridge.rs`) |
| fail → retry strategy per error class | `hexstrike_server.py:1661` | a `FallbackStrategy` enum consumed on non-zero exit in the tool runner |

### `mch-cyber` — workflow, swarm, evidence

| HexStrike | Location | `mch-cyber` target |
|---|---|---|
| `BugBountyWorkflowManager` / `CTFWorkflowManager` | `hexstrike_server.py:2447, 2795` | `hunt::HuntOrchestrator` (`crates/mch-cyber/src/hunt.rs`) — its `HuntPhase` state machine (Init→…→Report, hunt.rs:97) already supersedes the ad-hoc workflow dicts |
| `CTFTeamCoordinator` / parallel `_identify_parallel_tasks` | `hexstrike_server.py:3298, 4072` | `swarm::ChallengeSwarm` + `swarm::Coordinator` + `swarm::MessageBus` (`crates/mch-cyber/src/swarm.rs:112, 179, 314`) — HexStrike has *no* real swarm; the ingest validates the design we already have |
| `CVEIntelligenceManager`, `VulnerabilityCorrelator` | `hexstrike_server.py:5750, 8510` | threat-intel source feeding `hunt::Finding` / `consensus` scoring; `find_attack_chains` → `hunt::AttackChain` |
| exploit templates (`SQLiExploit`…) | `hexstrike_server.py:7393`+ | optional `mch-cyber` exploit-template library; keep as data, no inference |

### `mch-mcp` — registration and aggregation

HexStrike registers **one MCP tool per capability**, all owned by a single server, each proxying to a backend endpoint. `mch-mcp`'s `catalog::ToolCatalog` (`crates/mch-mcp/src/catalog.rs`) already does exactly the aggregation HexStrike lacks on the server side.

| HexStrike | Location | `mch-mcp` / `mch-kali` target |
|---|---|---|
| `FastMCP("hexstrike-ai-mcp")` + 151 `@mcp.tool()` | `hexstrike_mcp.py:277`+ | a native MCP **server** in `mch-kali` that registers each `KaliTool` wrapper as a tool; owner-namespaced (`kali__nmap`) so `ToolCatalog::build` merges it cleanly |
| implicit schema from type hints | `hexstrike_mcp.py:284` | `mch-mcp::message::Tool.input_schema` — hand-written `serde_json::json!` schemas per wrapper (we want explicit, not inferred) |
| stdio MCP front, HTTP backend | `hexstrike_mcp.py:5462` / `hexstrike_server.py` end | `mch-mcp::transport::StdioTransport` in front of an in-process tool executor — collapse HexStrike's two-process split into one binary |
| `use_recovery = True` default | `hexstrike_mcp.py` (every scan tool) | errors-as-data via `mch-mcp::message::CallToolResult { is_error }` + a fallback attempt before returning |

---

## What I take, and what I leave

**Take:**
1. **The `ToolEffectiveness` matrix** (`hexstrike_server.py:581`) — a data-driven `{target_type → tool → score}` table. It replaces `mch-kali`'s hand-coded `if fp.has(...)` pivots with one general selection function.
2. **Objective modes** (`quick`/`comprehensive`/`stealth`) as a first-class filter on tool selection (line 971).
3. **The tool→fallback map** (`_initialize_tool_alternatives`, line 1872) and per-error recovery strategies (line 1661) — self-healing execution, promoted into `ToolRegistry` + the tool runner.
4. **The two category taxonomies**: the 13 functional categories (`/health`, line 9023) and the 28 CTF categories (line 3673) — richer axes than `mch-kali`'s current four.
5. **Ordered attack patterns / playbooks** (line 715) as seed data for `TacticalPlan` templates.
6. **`TargetType` classification** (line 837) as a peer to `TechStack` in `TargetFingerprint`.
7. **The `/health` availability rollup** — per-category installed/missing, straight into `ToolRegistry`.

**Leave:**
1. **The two-process proxy** (`hexstrike_mcp.py` → HTTP → `hexstrike_server.py`). `mch-mcp` is one binary; a backend HTTP hop is a distribution detail we don't need.
2. **FastMCP's inferred schemas.** We hand-write explicit `input_schema` so the contract is ours and visible.
3. **String-concatenated shell commands** (`f"nmap {scan_type} …"`, line 10347). `mch-kali` builds argv arrays — no shell, no injection surface.
4. **The "AI" exploit/payload generators** (`AIExploitGenerator`, `AIPayloadGenerator`). They call no model — templating shipped as inference. Metadata only, not a dependency.
5. **`_detect_technologies`/`_detect_cms` heuristics** (lines 879/897) — it never makes an HTTP request (self-admitted stub). `mch-kali` fingerprints with `Whatweb`/`Nmap` and the `HeuristicAnalyzer` (`tactical.rs:135`).
6. **The README's "multi-agent" framing.** There is no agent loop in HexStrike; whatever swarm semantics we want live in `mch-cyber`, and the ingest should not import a mental model the code doesn't have.

The value in HexStrike is not the 150 tools (those are just `which nmap`) and not the proxy. It is the **curated selection matrix plus the fallback/recovery logic** — a static expert system for "given this target, which Kali tools, in what order, with what flags, and what to do when one fails." That is precisely `mch-kali`'s job, and the matrix is the piece our hand-coded `derive_actions` has been approximating.
