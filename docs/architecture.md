# McHarness Architecture

## Design law: The Whole

No visible seams. The agent does not experience tool use as a separate handshake — tools, memory, skills, and inference are compiled subsystems of one organism. Intent in, action out.

```
+-----------------------------------------------------------+
|                      McHarness Core (Rust)                |
|                                                           |
|  +---------------+   +-------------+   +--------------+   |
|  |  Agent Loop   |<->|   Memory    |<->|  Fusion Core |   |
|  | plan-act-     |   | persistent  |   | (skills,     |   |
|  | observe       |   | working     |   |  domain-     |   |
|  | (no seams)    |   | skill       |   |  addressed)  |   |
|  +-------+-------+   +-------------+   +--------------+   |
|          |                                                |
|  +-------v--------+   +---------------+                  |
|  |  Key Farm      |   |  Fission      |                  |
|  | key pool       |   |  (plugins,    |                  |
|  | rate telemetry |   |  one-command  |                  |
|  | model routing  |   |  updates)     |                  |
|  +-------+--------+   +---------------+                  |
|          |                                                |
|  +-------v----------------+  +------------------------+   |
|  | Model Layer            |  | Ingestor               |   |
|  | model-agnostic         |  | any OSS agent ->       |   |
|  | local (Ollama) + APIs  |  | mine + integrate       |   |
|  | leaderboard fallback   |  +------------------------+   |
|  +------------------------+  +------------------------+   |
+---------------------------+-------------------------------+
                            |
              +-------------v-------------+
              |  Speed Organs (C++)       |
              |  token streaming          |
              |  memory search            |
              |  embedding math           |
              +---------------------------+
```

## Subsystems

### Agent Loop (`core/src/loop/`)
The plan-act-observe cycle. Native, not scripted: tools are compiled-in organs, invoked without handshake overhead.

### Memory (`core/src/memory/`)
Three strata, compiled in:
- **Persistent** — across sessions (SQLite via the featherweight layer)
- **Working** — the live context of the current session
- **Skill** — the fused skill corpus, domain-indexed

### Key Farm (`core/src/keyfarm/`)
- One pool of keys; per-task model selection ("DeepSeek for design")
- The farm discovers every key that carries the chosen model and orders them as fallbacks
- Live rate-limit telemetry; pause/reroute before failure
- Leaderboard consultation: on model failure, select the next best proven model for the task domain

### Fusion (`core/src/fusion/`)
All skills ever, one core, addressed by domain. One command reverse-engineers and merges new skills.

### Fission (`core/src/fission/`)
Same pattern for plugins.

### Ingestor (`core/src/ingestor/`)
Feed any open-source agent; it mines capabilities and integrates them, logging lineage to the Forge Ledger.

### Speed Organs (`cpp/`)
C++ hot paths behind a safe FFI boundary: token streaming, memory search, embedding math.

### TUI (`tui/`)
Rich terminal interface: live Key Farm telemetry, model routing visualization, memory/skill activity. Dark, fast, alive.

## Featherweight doctrine (Omarchy philosophy)

The download is tiny. Every tool/capability ships as a placeholder manifest; on first use, the organ self-installs. Nothing paid for twice, nothing carried unused.

## Security team scope safety

Security specialist subsystems carry a hard precondition: an explicit authorization record for the target. No authorization, no action — enforced in code, not by prompt.
