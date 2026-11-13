# yt-dlp Ingestor Dossier

**Lineage:** yt-dlp (yt-dlp/yt-dlp) — Unlicense/public-domain, Python
**Source:** `forge/yt-dlp` (`yt_dlp/downloader/*`, `yt_dlp/networking/*`, `yt_dlp/utils/_utils.py`)
**Forge target:** `crates/mch-download` — the shared download engine used by the Ingestor (media) and model-weight fetchers
**Status:** `extracting` → `forged`

---

## What I read, and how

I cloned the repo (`forge/yt-dlp`) and read source, not READMEs. yt-dlp's extractors are a
firehose of per-site hacks; the durable engineering is in **`yt_dlp/downloader/`** — a small,
composable download engine that survives far worse networks than a datacenter ever will. Four
subsystems are worth stealing verbatim:

- **The manifest→fragment→checkpoint→resume session** — `downloader/fragment.py`, `downloader/hls.py`, `downloader/dash.py`
- **Retry/backoff semantics** — `downloader/common.py`, `downloader/http.py`, `utils/_utils.py::RetryManager`, `__init__.py::parse_sleep_func`
- **Auth refresh on 401/403** — `networking/common.py`, `networking/_urllib.py`, `YoutubeDL.py::_handle_extraction_exceptions`
- **The external-downloader hook** — `downloader/external.py`

Everything below points at a real file and line. Where I extrapolate I say so.

---

## Architecture

Two layers, cleanly separated: a **download loop** and a **transport**.

### Layer 1 — downloaders (`yt_dlp/downloader/`)

`FileDownloader` (`downloader/common.py:37`) is the base. It owns everything transport-agnostic:
the `.part` temp convention (`temp_name` :217 → `filename + '.part'`; `ytdl_filename` :229 → `.ytdl`),
rate limiting (`slow_down` :201), adaptive block sizing (`best_block_size` :181), the atomic
`try_rename`/`try_remove` helpers (`:259`, `:264`), and the retry-report surface (`report_retry` :410).
Subclasses implement exactly one method — `real_download` (`common.py:484`). `download()`
(`common.py:430`) is the public entry: it short-circuits on already-present files, honors `--sleep-interval`
(non-deterministic `random.uniform`, :473), then calls `real_download`.

Dispatch is `get_suitable_downloader` (`downloader/__init__.py:4`) + `PROTOCOL_MAP` (`:40`), keyed on
the resolved protocol (`http`, `http_dash_segments`, `m3u8`/`m3u8_native`, `f4m`, `ism`, `rtmp`,
`websocket_frag`, `mhtml`, …).

Two families of downloader:

| Family | Base | Members | Mechanism |
|---|---|---|---|
| Native | `HttpFD` (`http.py:23`), `FragmentFD` (`fragment.py:26`) | `HlsFD`, `DashSegmentsFD`, `F4mFD`, `IsmFD`, `MhtmlFD`, `HttpFD` | In-process HTTP via `ydl.urlopen` |
| External | `ExternalFD` (`external.py:37`) | `CurlFD`, `WgetFD`, `Aria2cFD`, `AxelFD`, `HttpieFD`, `FFmpegFD` | Shell out to a CLI, stream stderr |

`FragmentFD` is the hinge: every manifest-based format (HLS, DASH, F4M, ISM, MHTML) is a subclass,
and every external downloader inherits it too (`external.py:37`). One fragment engine, many front-ends.

### Layer 2 — transport (`yt_dlp/networking/`)

A `RequestHandler` ABC (`networking/common.py:149`) with pluggable backends (`_urllib.py:342`
`UrllibRH`, `_requests.py`, `_curlcffi.py`, plus an `ImpersonateRequestHandler` for TLS/JA3
fingerprint spoofing). `RequestHandler.send` (`:356`) → `_send`. Headers and the cookie jar are
resolved *per request* (`_get_headers` :269, `_get_cookiejar` :283, `_merge_headers` :260) — this
per-request freshness is what makes cookie/token rotation work (Crown Jewel 3).

---

## Crown Jewels

### 1. The DownloadSession: manifest → fragment → checkpoint → resume

A **fragment** is a dict: `{frag_index, url, decrypt_info, byte_range, media_sequence}` — see
`hls.py:219-225` and `dash.py:90-96`. A **manifest** is just a sequence of these, produced by
parsing (HLS: `hls.py:204-299`; DASH: `dash.py:74-96`). Fragments carry their own byte-range
(`byters` → HTTP `Range`, applied in `fragment.py:446-448`), AES-128 key material (`hls.py:265-282`),
and a monotonic `frag_index` that is the **resume cursor**.

The session state is a plain `ctx` dict, threaded through `download_and_append_fragments`
(`fragment.py:431`). The loop is: `download_fragment` → `append_fragment` → write checkpoint.

**The checkpoint is the crown jewel.** FragmentFD's own docstring (`fragment.py:41-59`) specifies the
`.ytdl` JSON sidecar:

```json
{"downloader": {"current_fragment": {"index": N}, "fragment_count": M, "extra_state": {...}}}
```

- `_write_ytdl_file` (`fragment.py:95`) is called **after every appended fragment**
  (`_append_fragment:146-152`), so the checkpoint is at most one fragment stale.
- `_read_ytdl_file` (`:82`) restores `ctx['fragment_index']` and `extra_state` (used by the WebVTT
  `pack_func` to carry cross-fragment timestamp/dedup state — `hls.py:319-401`).
- On success it is deleted (`_finish_frag_download:287`).

**Resume is two-layered.** Byte-level: `_prepare_frag_download` (`fragment.py:157`) stats the `.part`
file, and if `> 0` switches `open_mode` to `'ab'` (`:179-181`). Fragment-level: fragments whose
`frag_index <= ctx['fragment_index']` are skipped outright (`hls.py:213-214`, `dash.py:81-82`). Each
in-flight fragment also gets its own `%s-Frag%d` tempfile with its own resume offset
(`fragment.py:112`, `:120-122`).

**Consistency guards** (`fragment.py:189-205`): if the `.ytdl` is unparseable (`ytdl_corrupt`) or
`fragment_index > 0` but the file is empty (`is_inconsistent`), it warns and **restarts from zero**
rather than trust a torn state.

**Concurrency**: `concurrent_fragment_downloads` spins a `ThreadPoolExecutor` (`fragment.py:485-507`),
each worker on a `ctx.copy()`; multiple requested formats run in parallel via
`download_and_append_fragments_multiple` (`:367`) with one shared `interrupt_trigger` flag for clean
Ctrl-C.

### 2. Retry/backoff semantics

Everything retries through one iterator, `RetryManager` (`utils/_utils.py:5259-5313`):

```python
for retry in RetryManager(n_retries, error_callback):
    try:
        ...
    except SomeError as err:
        retry.error = err      # signals the manager
        continue
```

`_should_retry` (`:5274`) is simply `attempt <= retries`. After each failed yield the
`error_callback` fires (`:5292-5293`). Setting `retry.error = None` and continuing **re-yields** —
and because `NextFragment` does `retry.attempt -= 1` (`http.py:367-370`), a chunked-HTTP chunk
boundary does **not** consume a retry.

There are **four independent retry budgets**, each with its own policy:

| Budget | Default | Site |
|---|---|---|
| `retries` (whole HTTP request) | 10 (CLI) | `http.py:360` |
| `fragment_retries` (per fragment) | 10 (CLI) | `fragment.py:459`, `external.py:156` |
| `file_access_retries` | 3 | `common.py:241` |
| `extractor_retries` | — | `extractor/common.py:4065` |

**Backoff is a pluggable function, not a constant.** `--retry-sleep [TYPE:]EXPR` (`options.py:1036`)
is parsed by `parse_sleep_func` (`__init__.py:287-297`):

```python
# exp=START[:END[:BASE=2]]  →  min(start * base**n, limit)
# linear=START[:END[:STEP]] →  min(start + step*n, limit)
```

The type prefix (`http` default, plus `fragment`, `file_access`, `extractor`) routes the function to
that retry budget (`common.py:417` picks by `is_frag or 'http'`). The callable is invoked inside
`RetryManager.report_retry` (`utils/_utils.py:5295`), which also owns the **give-up** decision
(`:5298`): on `count > retries` it either calls the supplied `error` fn (`"… Giving up after N retries"`)
or re-raises — a *fatal* retry reports an error, a *non-fatal* one just warns and skips.

**What is retryable is explicit** (`http.py:148-192`): 5xx and `TransportError` → `RetryDownload`;
4xx (except 416) and `CertificateVerifyError` → raised immediately (no retry). **416** triggers
resume reconciliation (`:149`). A sustained under-rate triggers `ThrottledDownload` (`:315-323`) and
is retried. This is the discipline `mch-download` should copy: retry transport and server faults,
never retry a deterministic client error.

### 3. Auth refresh on 401/403

Two mechanisms, and honesty about the limit between them.

**(a) Cookie/token rotation — native downloaders get it for free.** The transport resolves the
cookie jar *per request* (`networking/common.py:283` `_get_cookiejar`, `:260` `_merge_headers`;
`UrllibRH._send` passes the live jar into `HTTPCookieProcessor`, `_urllib.py:392-403`). When the
server rotates a session cookie via `Set-Cookie` on the manifest or on fragment *k*, fragment *k+1*'s
request picks it up with no code in the download loop. Token-in-header rotation works the same way
if the extractor rebuilds headers per request.

**(b) Signed-URL expiry (403 with a stale token in the query) — re-extract, then resume.** Native
downloaders do **not** re-mint signed URLs themselves. The designed recovery is the `ReExtractInfo`
exception (`utils/_utils.py:1121`). `_handle_extraction_exceptions` (`YoutubeDL.py:1723-1753`) wraps
`extract_info` (`:1858`) and `process_ie_result` (`:2196`) in a `while True` loop that, on
`ReExtractInfo`, logs `Re-extracting data` and re-runs extraction. The magic is that the **`.ytdl`
checkpoint survived** (Crown Jewel 1), so the freshly-minted fragment list begins at
`current_fragment.index` — the retry is a *re-auth + resume*, not a restart. `dash.py:34` raises
`ReExtractInfo` directly when a URL-generator stream must be rebuilt; `YoutubeDL._wait_for_video`
(`:1790`) uses the same channel for "wait then retry".

**The gap, stated plainly:** external downloaders snapshot cookies *once* into a temp file when the
command is built (`external.py:132-140` `_write_cookies`) and never refresh mid-download. That is a
real limitation `mch-download` should close by keeping the cookie/token provider in-process and
re-snapshotting on child restart.

### 4. The external-downloader hook

`ExternalFD` (`external.py:37`) lets a battle-tested CLI (curl/wget/aria2c/ffmpeg/axel/httpie) do the
bytes while yt-dlp keeps progress, cookies, headers, and post-processing. Subclasses implement
`_make_cmd` (`curl.py`→`external.py:215`, `:281`, `:312`, `:265`, `:360`) or, for ffmpeg,
`_call_downloader` (`:400`).

Selection is name→class: `get_external_downloader` (`external.py:590`) resolves the
`--external-downloader` value; capability gates are `available` (`:96`, probes `-V`/`--version`),
`supports` (`:106`, protocol/`to_stdout`/`m3u8` feature checks), and `can_download` (`:115`). The
dispatcher consults the per-protocol pick first (`__init__.py:93-104`): `downloaders.get(shorten_protocol_name(protocol))`
falling back to `'default'`.

**Two operating modes** (`external.py:142-189`):

- **Whole-file** (`'fragments' not in info_dict`) — `_call_process` runs the CLI once, returns its
  exit code (`:148-152`).
- **Fragment-list** (`'fragments' in info_dict`) — the CLI is invoked under its *own*
  `RetryManager(fragment_retries)` (`:156-167`), then yt-dlp re-assembles the `%s-Frag%d` files,
  decrypts each, and appends (`:171-189`), deleting `tmpfilename.frag.urls` at the end (`:188`).

**Per-tool argument mapping** is where the value is: `--retry`/`--tries` come from the `retries`
param (`:238-242`, `:288-292`; `inf`→`2147483647` for curl, `inf`→`0` for wget); aria2c gets
multi-connection splitting `-x16 -j16 -s16 --min-split-size 1M` (`:313-316`); curl `>= 7.59` pipes
cookies via stdin `--cookie -` (`:218-228`). Extra flags are injected via `_configuration_args`
(`:127-130`) from `--external-downloader-args` (`common.py:66`).

---

## Rust Re-implementation Targets

New crate **`crates/mch-download`** — one engine, two consumers: the Ingestor (yt-dlp-style
manifest/fragment media) and **model-weight downloads** (direct HTTPS, HuggingFace `resolve/` URLs,
multi-GB `safetensors`/GGUF, HF `ETag`/resume). Both are "big file over a flaky link"; the fragment
model generalizes cleanly.

| Crown jewel | Python | `mch-download` target |
|---|---|---|
| Download session | `ctx` dict + `download_and_append_fragments` (`fragment.py:431`) | `struct DownloadSession { manifest: Manifest, cursor: u64, part: File }`, `async fn run(&mut self, fetch: impl FragmentFetch)` |
| Manifest → fragments | dict list (`hls.py:219`, `dash.py:90`) | `trait Manifest { fn fragments(&self) -> BoxStream<'_, Fragment> }`; `struct Fragment { index: u64, url: Url, range: Option<ByteRange>, key: Option<AesKey> }` |
| Checkpoint | `.ytdl` JSON (`fragment.py:41-59`, `:95`) | `#[derive(Serialize)] struct Checkpoint { current_fragment: u64, fragment_count: u64, extra_state: serde_json::Value }`; write after each append, `fsync`+atomic rename; delete on success |
| Resume (byte + fragment) | `open_mode='ab'` + skip `<= index` (`fragment.py:179`, `hls.py:213`) | open `.part` with `O_APPEND`; skip fragments `<= checkpoint.current_fragment`; per-fragment `<part>-Frag<idx>` offsets |
| Consistency guard | corrupt/inconsistent restart (`fragment.py:194-205`) | validate checkpoint vs part size; on mismatch log + restart from 0 |
| Concurrency | `ThreadPoolExecutor` (`fragment.py:485`) | `tokio::task::JoinSet` bounded by `Semaphore(concurrent_fragments)` |
| Retry loop | `RetryManager` (`utils/_utils.py:5259`) | `async fn retry<T,E>(policy: &Backoff, budget: RetryBudget, f: impl FnMut() -> Result<T,E>) -> Result<T,E>` |
| Backoff | `parse_sleep_func` exp/linear (`__init__.py:287`) | `enum Backoff { Constant(Duration), Linear{start,step,end}, Exponential{base,start,end} }` via the `backoff`/`tower` crate |
| Retry budgets | `retries`/`fragment_retries`/`file_access_retries` (`common.py`) | `struct RetryBudgets { request: u32, fragment: u32, file_access: u32 }` |
| Retryable-classification | 5xx/transport retry, 4xx fatal, 416 reconcile (`http.py:148`) | `fn classify(status) -> Retry | ReconcileRange | Fatal` on the error type |
| Auth refresh | per-request cookiejar (`common.py:283`) + `ReExtractInfo` re-extract (`YoutubeDL.py:1723`) | `#[async_trait] trait TokenProvider { async fn headers(&self) -> HeaderMap; async fn on_unauthorized(&self, req) -> RefreshOutcome }`; loop: on 401/403 call `on_unauthorized`, retry the fragment; if the manifest itself is expired, `RefreshOutcome::ReExtract` → re-fetch manifest and **seek the checkpoint**, never restart the file |
| External hook | `ExternalFD` (`external.py:37`) | `#[async_trait] trait ExternalDownloader { fn command(&self, url, dest, opts) -> Command; }` with `Aria2c`, `Curl`, `Ffmpeg` impls; keep cookies **live** (re-export on child restart — closes yt-dlp's gap) |
| Rate limit / block sizing | `slow_down`/`best_block_size` (`common.py:201`, `:181`) | `TokenBucket` limiter + adaptive read buffer |

**Model-weight-specific additions** (the generalization the Ingestor doesn't need):
`If-Range`/`ETag`-guarded resume so a re-published weight file is detected rather than silently
stiched; whole-file `sha256` verification of the assembled artifact before the atomic
`.part`→final rename (yt-dlp verifies only fragment MHTML MD5 — `downloader/mhtml.py`); and an
`Xet`/HF-LFS-aware fragment source for chunked weight servers.

### What I take, and what I leave

**Take:** the four-budget retry model; pluggable exp/linear backoff *per retry type*; the
fatal-vs-non-fatal give-up split; the `.ytdl` checkpoint written per-fragment with atomic rename; the
two-layer (byte + fragment) resume; the re-extract-then-resume recovery for signed URLs; the external
downloader as a first-class backend.

**Leave:** the `ctx` dict discipline (a typed `DownloadSession` struct in Rust); the
`retry.error = None` iterator idiom (use `Result` + a `retry` combinator); per-call `ctx.copy()`
(Rust ownership); and yt-dlp's external-cookie-snapshot limitation (fix it, don't port it).

---

## File reference

| Capability | File | `mch-download` module |
|---|---|---|
| Downloader base / `.part` / file-access retry | `downloader/common.py` | `session`, `atomic` |
| Fragment session / `.ytdl` checkpoint / resume | `downloader/fragment.py` | `session`, `checkpoint` |
| Native HTTP / range / 416 / chunked | `downloader/http.py` | `http` |
| HLS parse → fragments | `downloader/hls.py` | `manifest::hls` |
| DASH parse → fragments | `downloader/dash.py` | `manifest::dash` |
| External downloader hook | `downloader/external.py` | `external` |
| Dispatch / protocol map | `downloader/__init__.py` | `registry` |
| Retry iterator / report | `utils/_utils.py` (`RetryManager`, `:5259`) | `retry` |
| Backoff expression parser | `__init__.py` (`parse_sleep_func`, `:287`) | `backoff` |
| Per-request headers / cookiejar | `networking/common.py`, `networking/_urllib.py` | `auth` |
| Re-extract loop (signed-URL refresh) | `YoutubeDL.py` (`:1723-1753`) | `auth::recover` |
| Fragment integrity check | `downloader/mhtml.py` | `verify` |

---

## Open questions

- [ ] Does HF's `resolve/` redirect + signed S3 URL need the `ReExtractInfo` pattern (re-fetch manifest) or a simple `If-Range` retry? Prototype both.
- [ ] `fsync` cadence on the checkpoint — per fragment is yt-dlp's choice; measure the cost on multi-GB weight downloads (checkpoint every N MB instead?).
- [ ] Share `mch-download`'s `TokenProvider` with `mch-transport` for auth, or keep it download-local?
