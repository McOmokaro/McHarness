# mch-brain: Native Rust Memory Graph for LLM Agents

**Sources:**
- **LycheeMem** (HITsz-TMG/LycheeMem) — Lightweight long-term memory for LLM agents
- **LycheeMemory** (owoakuma/LycheeMemory) — Dynamic long-context reasoning via compressed memory

**Forge Target:** `crates/mch-brain`
**Status:** Forged

---

## Overview

mch-brain implements a **four-store memory architecture** with hierarchical consolidation, action-aware retrieval, and MCP compatibility. It's forged from LycheeMem's production-grade memory infrastructure for LLM agents.

**Key insight:** Memory is not a flat vector store. It's a **typed, hierarchical graph** with action-facing metadata, usage statistics for RL, and a background Curator daemon that continuously consolidates experience into structured knowledge.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        mch-brain                                │
├──────────────┬──────────────┬──────────────┬──────────────────┤
│ Working Mem  │ Semantic Mem │ Procedural   │ Visual Mem       │
│ (Episodic)   │ (Typed Store)│ (Skills)     │ (Multimodal)     │
├──────────────┼──────────────┼──────────────┼──────────────────┤
│ Session turns│ 7 Record     │ Skill entries│ VLM captions     │
│ Summaries    │ types        │ HyDE retrieval│ CLIP embeddings │
│ Token budget │ SQLite+LanceDB│ Usage stats  │ Ebbinghaus       │
│ (70%/90%)    │ Hierarchy    │              │ forgetting       │
└──────────────┴──────────────┴──────────────┴──────────────────┘
         │              │              │              │
         └──────────────┴──────────────┴──────────────┘
                          │
                    ┌─────▼─────┐
                    │  Curator  │  Background consolidation daemon
                    │  Daemon   │  Record Fusion → CompositeRecords
                    └───────────┘
```

### Four Memory Stores

| Store | Type | Content | Storage | Key Innovation |
|-------|------|---------|---------|----------------|
| **Working** | Episodic | Session turns, summaries | In-memory + token budget | Dual-threshold (70%/90%) compression |
| **Semantic** | Typed | 7 MemoryRecord types | SQLite (FTS5) + LanceDB (ANN) | Record Fusion → Hierarchical tree |
| **Procedural** | Skills | Skill entries + HyDE | SQLite + LanceDB | Hypothetical Document Embeddings |
| **Visual** | Multimodal | VLM + CLIP embeddings | SQLite + LanceDB + FS | Dual-channel retrieval + forgetting |

---

## Working Memory (Episodic)

### Dual-Threshold Token Budget

```
┌─────────────────────────────────────────────────────────────┐
│                    Token Budget                              │
├─────────────────────────────────────────────────────────────┤
│ 0%                                    70%        90%    100% │
│ ████████████████████████████████████████████████████████████ │
│         Normal              Warn (async)      Block (sync)  │
│                          Pre-compress        Compress now   │
└─────────────────────────────────────────────────────────────┘
```

- **Warn (70%)**: Async background pre-compression, current request not blocked
- **Block (90%)**: Pipeline pauses, flushes older turns to compressed summary

### Compression Output

```
CompressedHistory {
  summaries: [
    SummaryAnchor { turn_range: (0, 5), summary: "User asked about database backups...", tokens: 45 },
    SummaryAnchor { turn_range: (6, 10), summary: "Discussed pg_dump vs pg_basebackup...", tokens: 38 }
  ],
  recent_turns: [Turn{role:User, content:"What about encryption?", tokens: 12}, ...]
}
```

Both summary anchors (distilled past) and raw recent turns (verbatim) passed downstream.

---

## Semantic Memory (Typed Action Store)

### 7 MemoryRecord Types

| Type | Description | Example |
|------|-------------|---------|
| `fact` | Objective facts | "PostgreSQL runs on port 5432" |
| `preference` | User preferences | "Prefers pg_dump over pg_basebackup" |
| `event` | Specific events | "Database backup failed on 2026-01-15" |
| `constraint` | Must-respect conditions | "Backups must encrypt at rest" |
| `procedure` | Reusable methods | "Steps for point-in-time recovery" |
| `failure_pattern` | Failed paths | "pg_basebackup fails without replication slot" |
| `tool_affordance` | Tool capabilities | "pg_dump supports --format=custom" |

### Record Structure

```rust
MemoryRecord {
    record_id: SHA256(normalized_text),  // Idempotent deduplication
    memory_type: Fact,
    normalized_text: "User prefers pg_dump for backups",
    semantic_text: "Prefers pg_dump for backups",
    metadata: ActionMetadata {
        tool_tags: ["pg_dump", "backup"],
        constraint_tags: ["encryption"],
        affordance_tags: ["compression", "parallel"],
        confidence: 0.92,
    },
    usage: UsageStats { retrieval_count: 15, action_success_count: 12, ... },
    child_composite_ids: [...],  // Hierarchy links
}
```

### Record Fusion Engine (Zero-LLM Consolidation)

```
New Records → ANN Search (cosine > 0.85) → Deduplication
                      ↓
               Clustering (cosine > 0.75) → Union-Find
                      ↓
         CompositeRecord from cluster representative
         (merged metadata, aggregated usage, hierarchy links)
                      ↓
              Hierarchy rounds → composite → composite abstractions
```

**Result:** Hierarchical memory tree instead of flat bag of summaries.

### Action-Aware Hierarchical Retrieval (4 Modules)

```
┌─────────────────────────────────────────────────────────────────┐
│                    RETRIEVAL PIPELINE                           │
├─────────────────────────────────────────────────────────────────┤
│ Module 1: Compact Semantic Encoding (LLM)                      │
│   Typed extraction → Decontextualization → Action metadata     │
├─────────────────────────────────────────────────────────────────┤
│ Module 2: Record Fusion (Zero-LLM, embedding math)             │
│   Deduplication → Clustering → Composite → Hierarchy          │
├─────────────────────────────────────────────────────────────────┤
│ Module 3: Action-Aware Hierarchical Retrieval                  │
│   1. ANN pre-filter → Top-20 CompositeRecords                 │
│   2. Single LLM call: Relevance judgement + expansion flags   │
│   3. Tree expansion: Composite → Atomic records               │
│   4. Reflection loop: FTS + Vector + Episodic recall          │
├─────────────────────────────────────────────────────────────────┤
│ Module 4: Candidate Aggregation + Context Enrichment           │
│   Tier ranking: Composite > Tree Expansion > Supplementary    │
│   Episodic context: Original dialogue excerpts appended       │
└─────────────────────────────────────────────────────────────────┘
```

---

## Procedural Memory (Skill Store)

### HyDE Retrieval (Hypothetical Document Embeddings)

```
User Query: "How to backup PostgreSQL to S3?"
       │
       ▼
LLM generates hypothetical ideal answer:
"To backup PostgreSQL to S3, use pg_dump with custom format,
 pipe to aws s3 cp, use --format=custom for compression..."
       │
       ▼
Embed hypothetical → Query vector
       │
       ▼
Vector similarity against skill intents
       │
       ▼
Top-k skills (even with vague user phrasing)
```

### Skill Entry

```rust
SkillEntry {
    intent: "pg_dump backup to S3",
    doc_markdown: "# pg_dump to S3\n\n1. pg_dump -Fc dbname | aws s3 cp - s3://bucket/backup.dump\n2. ...",
    embedding: [...],
    usage_count: 42,
    preconditions: ["aws cli configured", "pg_dump installed"],
}
```

---

## Visual Memory (Multimodal)

### Dual Embedding

| Channel | Model | Purpose |
|---------|-------|---------|
| Caption | VLM (qwen-vl-max) | Text → Visual retrieval |
| Visual | CLIP (ViT-B/32) | Image → Image retrieval |

### VisualRecord

```rust
VisualRecord {
    caption: "Screenshot of pgAdmin showing backup dialog",
    scene_type: Ui,
    caption_embedding: [...],   // Text vector
    visual_embedding: [...],    // CLIP vector
    importance_score: 0.8,      // 0.0-1.0, governs forgetting
    image_hash: "sha256...",    // Deduplication
}
```

### Ebbinghaus Forgetting Curve

```
decay = 0.5^(t / t_half_eff)

t_half_eff = base_half_life * (1 + 3 * importance)
- importance=0.0 → 7 days half-life
- importance=1.0 → ~30 days half-life

Each retrieval: +30% decay resistance
Max TTL: 90 days
```

---

## Pipeline: 5-Stage Agent Loop

```
┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐
│ 1. WMManager │   │ 2. SearchCoord   │   │ 3. Synthesizer   │
│              │   │                  │   │                  │
│ Token budget │──▶│ Recent context   │──▶│ LLM-as-Judge     │
│ Compression  │   │ ActionState      │   │ Score 0-1        │
│ Render       │   │ RetrievalPlan    │   │ Fuse → context   │
└──────────────┘   └──────────────────┘   └──────────────────┘
       │                    │                     │
       ▼                    ▼                     ▼
┌──────────────┐   ┌──────────────────┐   ┌──────────────────┐
│ 4. Reasoning │   │ 5. Consolidator  │   │   (Background)   │
│              │   │                  │   │                  │
│ Final answer │   │ Compact ingest   │   │ Zero-LLM fusion  │
│ Append turn  │   │ Skill extraction │   │ Periodic         │
└──────────────┘   └──────────────────┘   └──────────────────┘
```

### Stage Details

| Stage | Component | LLM Calls | Purpose |
|-------|-----------|-----------|---------|
| 1 | WMManager | 0 | Token budget, compression |
| 2 | SearchCoordinator | 1 (planning) + retrieval | Plan + retrieve |
| 3 | SynthesizerAgent | N (judge each fragment) | Filter + fuse |
| 4 | ReasoningAgent | 1 | Generate answer |
| 5 | ConsolidatorAgent | 1 (per session) | Background consolidation |

---

## Curator Daemon (Background)

### Consolidation Loop

```rust
// Periodic (5 min) + triggered
loop {
    // 1. Compact consolidation per session
    //    Typed extraction → Decontextualization → Action metadata
    //    → Write MemoryRecords → Record Fusion (dedup → cluster → composite)
    
    // 2. Skill extraction
    //    Identify successful tool patterns → Add skill entries
    
    // 3. Hierarchy rounds
    //    Composite → Composite abstractions
}
```

### Forgetting Pass (Hourly)

- Visual: Ebbinghaus decay with importance-weighted half-life
- Semantic: Could decay usage stats, soft-expire old records

---

## Storage: SQLite + LanceDB

| Layer | Purpose | Technology |
|-------|---------|------------|
| Relational + FTS5 | Metadata, full-text search | SQLite |
| Vector ANN | Embedding similarity search | LanceDB |
| Filesystem | Raw images (visual memory) | Local FS |

### Schema Highlights

```sql
-- Memory records
CREATE TABLE memory_records (
    record_id TEXT PRIMARY KEY,
    memory_type TEXT NOT NULL,
    normalized_text TEXT NOT NULL,
    semantic_text TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    usage_json TEXT NOT NULL,
    session_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    embedding BLOB  -- LanceDB handles vectors
);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE memory_records_fts USING fts5(...);
```

---

## MCP Compatibility

mch-brain exposes an HTTP MCP endpoint at `http://localhost:8000/mcp`:

| Tool | Purpose |
|------|---------|
| `lychee_memory_smart_search` | Default one-shot recall (compact mode) |
| `lychee_memory_search` | Raw retrieval for debugging |
| `lychee_memory_append_turn` | Mirror external host turns |
| `lychee_memory_consolidate` | Trigger consolidation |

```json
{
  "mcpServers": {
    "mch-brain": {
      "url": "http://localhost:8000/mcp"
    }
  }
}
```

---

## Integration Points

| McHarness Crate | Integration |
|-----------------|-------------|
| `mch-loop` | Drive autonomous loops with memory context |
| `mch-browser` | Store browsing facts, procedures, visual memories |
| `mch-mcp` | Expose memory as MCP tools |
| `mch-scope` | Gate consolidation triggers, skill extraction |
| `mch-audit` | Log every memory operation for reproducibility |
| `mch-rea` | Store REA findings as semantic/procedural records |

---

## Files Reference

| File | Description |
|------|-------------|
| `crates/mch-brain/src/types.rs` | Core types (RecordId, MemoryType, etc.) |
| `crates/mch-brain/src/working.rs` | Working memory (token budget, compression) |
| `crates/mch-brain/src/storage.rs` | SQLite + LanceDB storage |
| `crates/mch-brain/src/retrieval.rs` | Action-aware hierarchical retrieval |
| `crates/mch-brain/src/skills.rs` | Skill store + HyDE |
| `crates/mch-brain/src/visual.rs` | Visual memory + Ebbinghaus |
| `crates/mch-brain/src/curator.rs` | Background consolidation daemon |
| `crates/mch-brain/src/pipeline.rs` | 5-stage agent pipeline |
| `crates/mch-brain/src/memory.rs` | Memory tree structures |
| `docs/ingestor/brain.md` | This document |

---

## Open Questions / Future Work

- [ ] **Embedded fast triage**: Add local embedding model (ONNX) for zero-RTT retrieval
- [ ] **RL-ready signals**: Export usage stats for policy gradient training
- [ ] **Multi-tenancy**: Session/user isolation with shared semantic memory
- [ ] **Streaming consolidation**: Incremental fusion without full recomputation
- [ ] **Cross-agent memory**: Shared semantic memory across agent instances
- [ ] **Benchmark**: LoCoMo, LongMemEval, MSC-MemFuse evaluation