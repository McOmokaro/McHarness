# DeepInfer Ingestor Dossier

**Lineage:** DeepInfer (deepseek-ai/DeepInfer) — Apache-2.0, Python + CUDA/C++ kernels. Cloned to `forge/DeepInfer`.
**Status:** `forged` → wrap + extract below.
**Forge target:** `mch-keyfarm` (local-serving backend), `mch-gate` (structured-output contracts), `mch-cyber` (deterministic, batch-invariant evidence).

## What it is

DeepInfer is **DeepSeek's high-performance inference framework** — the engine behind
their production serving, open-sourced for offline inference. It supports
`DeepSeek-V4.1-Flash`. It is the local-heavy-model tier that McHarness's
"Low Fuel" router escalates to, so its scheduler + constrained-decoding semantics
are exactly the crown jewels we need for a native key-farm backend.

## Architecture

Python package `deepinfer/`:

| Path | Role |
|---|---|
| `engine/llm_engine.py`, `engine/arg_utils.py`, `engine/engine_type.py` | The async inference engine + config |
| `engine/stats_logger.py`, `engine/profile_server.py` | Metrics + profiling |
| `model_executor/layers/tilelang_ops/` | Custom TileLang kernels |
| `distributed/` | Attention DP / balanced CP, MoE EP + EPLB |
| `structure_output/` | **Constrained decoding**: `compiler.py`, `matcher.py`, `mask.py`, `so_matcher_builder.py` |
| `sampling_params.py` | `SamplingParams` — incl. `so_args` (schema_type/schema/trigger_token_ids) and deterministic `seq_seed` |
| `entrypoints/llm.py` | The `LLM(...)` entrypoint (vLLM-derived API) |
| `worker/`, `ve/`, `hugging_face/` | Worker loop, verification, HF model loading |

Kernels are external DeepSeek repos: **FlashMLA, DeepGEMM, DeepEP, TileKernels,
DeepSelect** (see `README.md`).

## Crown Jewels

1. **Zero-overhead async batch scheduler** — `engine/llm_engine.py` overlaps CPU
   batch scheduling with device execution ("asynchronous scheduling, enabled by
   default"). This is the throughput core any key-farm backend wants.
2. **Structured outputs / constrained decoding** — `structure_output/compiler.py`
   compiles a **regex, JSON schema, or EBNF grammar** into a token mask
   (`mask.py` + `matcher.py` + `so_matcher_builder.py`), applied via
   `SamplingParams.so_args` with optional `trigger_token_ids` (free generation until
   a trigger sequence appears, then the grammar bites). This is a *native* output
   contract — no post-hoc repair loop.
3. **Deterministic + batch-invariant inference** — `DEEPINFER_BATCH_INVARIANT=1`
   makes outputs identical across batch compositions, and `seq_seed` gives
   reproducible sampling. Identical input → identical bytes, which is what an
   audit/attestation path requires.
4. **Speculative decoding (DSpark) + confidence-scheduled verification** —
   `DEEPINFER_ENABLE_MTP_VARLEN`, `DEEPINFER_USE_NEXTN_DECODE` (draft length ≤ 5):
   accelerate decoding then verify by confidence. Direct low-latency optimization.
5. **SWA bounded replay** — `DEEPINFER_YOCO_PREFILL_TRUNCATION_MODE=compact`,
   `_KEEP_TOKENS` — replay only a bounded suffix of the prefix to cut prefill work.
6. **Parallelism strategies** — Attention DP / balanced CP, MoE EP + EPLB
   (`distributed/`) for scale-out.

## Rust Re-implementation Targets

- **`mch-keyfarm` (or new `mch-infer`):** model the engine as a sidecar process the
  Key Farm launches for the "heavy tier"; the Rust side holds the `SamplingParams`
  surface (temperature/top_p/top_k/max_tokens/**so_args/seq_seed**) and issues
  requests to `entrypoints/llm.py`'s HTTP/server boundary. We do **not** reimplement
  the CUDA kernels in Rust — we wrap the engine and own routing, budget, and
  fallback (consistent with the Low-Fuel design and the existing key pool).
- **`mch-gate`:** port the `structure_output/` idea as a **compiled output contract**:
  a `Requirement::MatchesSchema(schema)` gate that compiles JSON-schema/EBNF to a
  validator and fails closed when a model response violates it — the same role
  Cloudflare's `report-schema.json` plays for findings.
- **`mch-cyber`:** adopt batch-invariant + deterministic-seed semantics so a produced
  artifact (e.g. a generated exploit or a report) is **reproducible byte-for-byte**
  and therefore attestable with the existing Ed25519 signing path.

## Take / Leave

- **Take:** the async-scheduler contract; the schema/regex/EBNF constrained-decoding
  pipeline; deterministic + batch-invariant semantics; the speculative-decoding
  budget knobs; the clean `LLM()` entrypoint API.
- **Leave:** the CUDA/C++ kernels and the DeepSeek-specific MoE/EPLB distributed
  stack — McHarness wraps the engine rather than re-forging the GPU math.
