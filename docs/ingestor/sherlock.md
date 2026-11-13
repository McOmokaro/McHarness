# Sherlock Ingestor Dossier

**Lineage:** `sherlock-project/sherlock` v0.16.2 (MIT) — Python OSINT username enumerator.
Commit `c588d7c8b476df9f9c84912a05d1430fb8cdcdae` (2026-11-01) cloned to
`forge/sherlock/`. The project *is* its manifest: the crown jewel is
`sherlock_project/resources/data.json`, a flat 104 KB JSON of **481 social-network
detection recipes**, served live at `https://data.sherlockproject.xyz`. Entry point
`sherlock_project.sherlock:main` (`pyproject.toml [tool.poetry.scripts]`). Core deps:
`requests`, `requests-futures` (threadpool), `pandas`/`openpyxl` (xlsx export),
`PySocks`, `stem` (Tor), `colorama`, `certifi`. No database, no daemon — a one-shot
CLI that fans a username across hundreds of HTTP probes and classifies each hit.

---

## Architecture

Five modules, one data file, 1,614 Python LOC total.

| File | LOC | Role |
|------|-----|------|
| `sherlock_project/resources/data.json` | — | 481 site definitions + top-level `$schema` key |
| `sherlock_project/resources/data.schema.json` | — | JSON Schema 2020-12 contract for the manifest |
| `sherlock_project/sites.py` | 258 | `SiteInformation` / `SitesInformation` — manifest loader |
| `sherlock_project/sherlock.py` | 934 | Concurrency engine + `main()` CLI |
| `sherlock_project/result.py` | 88 | `QueryStatus` enum + `QueryResult` |
| `sherlock_project/notify.py` | 282 | `QueryNotify` base + `QueryNotifyPrint` (TTY output) |

Supporting machinery: `devel/site-list.py` (renders `sites.mdx`), `devel/summarize_site_validation.py`,
`tests/{test_manifest,test_probes,test_validate_targets,test_ux}.py`,
`.github/workflows/{exclusions,update-site-list,validate_modified_targets,regression}.yml`,
`.actor/` (Apify wrapper), `docs/removed-sites.md` (rejected targets log).

### Site / data model (`data.json` — the whole system)

`data.json` is a **single flat object**: top-level keys are human-readable site
names; the only reserved key is `$schema`. `data.schema.json` enforces
`patternProperties "^(?!\\$).*?$"` with `additionalProperties: false`.

Every target **requires** `url`, `urlMain`, `errorType`, `username_claimed`.
Optional keys: `urlProbe`, `regexCheck`, `isNSFW`, `headers`, `request_payload`,
`request_method` (`GET|POST|HEAD|PUT`), `__comment__` (unparsed), `tags`
(`adult`/`gaming`), and the detection payloads `errorMsg`/`errorCode`/`errorUrl`/`response_url`.

`url` carries a `{}` placeholder for the username (e.g. GitHub
`https://www.github.com/{}`); `urlProbe` overrides the existence-check URL when the
public profile path differs (e.g. Instagram probes `imginn.com/{}`). `urlMain` is the
site root; `username_claimed` is a known-live handle used as each target's built-in
self-test probe.

**Detection is a discriminated union on `errorType`** (string *or* list — types combine):
- `message` → search `errorMsg` (string *or* list of substrings) in the response body.
  Absent ⇒ `CLAIMED`, present ⇒ `AVAILABLE`.
- `status_code` → compare `r.status_code` against `errorCode` (int *or* list); a
  match, or any status outside 2xx/3xx, ⇒ `AVAILABLE`, else `CLAIMED`. Pure
  `status_code` targets use `HEAD` to skip the body.
- `response_url` → redirects disabled; 2xx ⇒ `CLAIMED`, else `AVAILABLE`.

Schema `` dependencies `` enforce that `errorMsg`/`errorUrl`/`errorCode` appear only
with the matching `errorType`. Live distribution across the 481 targets:
`status_code` 327, `message` 127, `response_url` 27. Feature usage: `regexCheck` 95,
`errorMsg` 127, `urlProbe` 52, `request_method` 27, `errorUrl` 28, `isNSFW` 19
(hence 19 NSFW-gated sites), `headers` 7, `errorCode` 6, `request_payload` 3.

### Loader (`sites.py`)

`SitesInformation(data_file_path, honor_exclusions, do_not_exclude)` accepts a local
path *or* an `http(s)` URL; when none is given it pulls the **live** `MANIFEST_URL`
`https://data.sherlockproject.xyz` so users never run stale data. It pops `$schema`,
then optionally fetches `EXCLUSIONS_URL`
(`raw.githubusercontent.com/.../refs/heads/exclusions/false_positive_exclusions.txt`)
and drops every named key — the community false-positive kill-list, maintained on a
separate git branch and honored at runtime. For each entry it builds a
`SiteInformation(name, urlMain, url, username_claimed, information, isNSFW)`; a
missing required attribute raises `ValueError`, a malformed non-dict target is
skipped with a warning. `username_unclaimed` defaults to `secrets.token_urlsafe(32)`.
Helpers: `remove_nsfw_sites()`, `site_name_list()`, `__iter__`, `__len__`.

### Concurrent-check engine (`sherlock.py`)

`SherlockFuturesSession(FuturesSession)` subclasses `requests-futures` and installs a
`response` hook that records `resp.elapsed` via `time.monotonic()` for per-site
timing. `sherlock()` sets `max_workers = min(20, len(site_data))` and runs a
**two-pass future/resolve schedule**:
1. **Dispatch pass** — for every site build the request (`session.get/head/post/put`
   chosen from `errorType`/`request_method`), spoof a Firefox `User-Agent`, merge
   per-site `headers`, interpolate `{}` into `url`/`urlProbe`/`request_payload`
   (`interpolate_string` recurses dicts/lists), and stash the returned future on
   `net_info["request_future"]`. A `regexCheck` miss short-circuits to
   `QueryStatus.ILLEGAL` *without* a request. `response_url` sites set
   `allow_redirects=False`. Proxies (`--proxy`) are applied per-request.
2. **Resolve pass** — `get_response()` pulls `future.result()` and maps exceptions
   (`HTTPError`, `ProxyError`, `ConnectionError`, `Timeout`, `RequestException`,
   `UnicodeError`) to an `error_context`; classify by `errorType`, after first
   checking the **WAF fingerprint table** (`WAFHitMsgs`: Cloudflare spinner/error
   page, Cloudfront `AwsWafIntegration.forceRefreshToken`, PerimeterX identifiers)
   ⇒ `QueryStatus.WAF`.

`main()` adds: `--local`, `--nsfw`, `--ignore-exclusions`, `--site`, `--timeout`,
`--proxy`, `--json` (local file, URL, or a **PR number** resolved to that PR's raw
`data.json`), `--browse`, CSV/XLSX/TXT export (pandas), `--dump-response`, and `{?}`
multi-username expansion (`{?}` → `_`/`-`/`.`). A GitHub release check prints an
update banner.

### Result model (`result.py`)

`QueryStatus(Enum)`: `CLAIMED` (detected), `AVAILABLE` (not found), `UNKNOWN` (error),
`ILLEGAL` (username fails `regexCheck`), `WAF` (bot-blocked). `QueryResult(username,
site_name, site_url_user, status, query_time, context)`. The engine returns
`results_total: {site_name: {url_main, url_user, status: QueryResult, http_status,
response_text}}`. `notify.py` streams these to the TTY via `QueryNotifyPrint`
(green `+` for claimed, red `-` otherwise) and counts hits.

---

## Crown Jewels

1. **The 481-recipe manifest** — a schema-validated, CI-gated, live-served corpus of
   detection rules. The real IP is the *data curation + false-positive control loop*,
   not the HTTP plumbing. `data.schema.json` + `test_manifest.py` +
   `validate_modified_targets.yml` make every target contributor-tested.
2. **`errorType` discriminated-union detection** — three orthogonal signals
   (`message`/`status_code`/`response_url`), each a string *or* list, so a target can
   AND/OR multiple rules. This single abstraction lets 481 heterogeneous sites share
   one classifier (~90 LOC, `sherlock.py:378-453`).
3. **Runtime false-positive exclusions** — `EXCLUSIONS_URL` on a dedicated branch,
   applied at load time; a decoupled kill-switch for bad targets without redeploying.
4. **`requests-futures` two-pass concurrency (20 workers)** with a monotonic timing
   hook — dispatch-all-then-resolve so latency hides behind the pool.
5. **WAF fingerprint table** (`WAFHitMsgs`) — hard-won anti-bot signatures promoted
   from false positives into a classifier state.
6. **Per-target self-test (`username_claimed`)** and **live manifest pull** — the
   tool validates its own rules and always runs current data.

---

## Rust Re-implementation Targets

Map to **`crates/mch-cyber`** (recon) and **`crates/mch-kali`** (OSINT tooling). Both
already ship `serde`/`schemars`, `tokio`, `reqwest` (mch-cyber), `regex`, `dashmap`,
`parking_lot` — the ports are dependency-clean.

- **Manifest data model** → new `crates/mch-cyber/src/recon/username.rs`. Port
  `data.schema.json` to `#[derive(Serialize, Deserialize, JsonSchema)] struct
  SiteDef { url, url_main, url_probe, error_type, error_msg, error_code, error_url,
  regex_check, headers, request_method, request_payload, username_claimed, is_nsfw,
  tags }` (enums for `ErrorType`, `HttpMethod`); reuse the 481-entry `data.json` verbatim.
- **Concurrent-check engine** → replace `FuturesSession` with
  `futures::stream::iter(sites).map(probe).buffer_unordered(20)` (or a tokio
  `JoinSet`). Reuse mch-cyber's `reqwest` client; port `interpolate_string`,
  `regexCheck` gating, the `errorType` classifier, and the WAF fingerprint table into
  a `SherlockSweep` type.
- **Result model** → `#[derive(Serialize, JsonSchema)] enum QueryStatus { Claimed,
  Available, Unknown, Illegal, Waf }` + `QueryResult`; serialize the
  `{url_main, url_user, status, http_status}` row shape.
- **Recon wiring** → invoke from `crates/mch-cyber/src/hunt.rs` `HuntPhase::Recon`
  (RECON phase, `hunt.rs:679`) as a username-enumeration action; surface as a
  `ToolCategory::Recon` entry alongside `Subfinder`/`Amass` in
  `crates/mch-kali/src/tools.rs` (add `KaliTool::Sherlock` or a pure-Rust tactical
  action in `crates/mch-kali/src/tactical.rs`).
- **Evidence/attestation** → wrap hits with mch-cyber `evidence::EvidenceBundle` and
  Ed25519 sign via `mch-cyber/src/signing.rs` (or `mch-kali/src/attest.rs`
  `attest_output`), so an OSINT sweep is cryptographically provable.
- **Exclusions** → port `EXCLUSIONS_URL` runtime kill-list into an
  `mch-cyber` recon config field.
