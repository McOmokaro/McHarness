# McHarness Agent Registry

**Source:** AI Income Challenge frameworks + Ideated agents for non-US founder pain points.

---

## Core Framework Agents (from Iman's Videos)

| Agent | File | Purpose | Status |
|---|---|---|---|
| **Segway Test Agent** | `segway-test-agent.md` | 3-question filter: Pothole vs Well | ✅ Defined |
| **Pain Extractor Agent** | `pain-extractor-agent.md` | Deep-dive into validated pain, quantify cost, define MVP | ✅ Defined |
| **Lead Gen Agent** | `lead-gen-agent.md` | Asian Empire: Places API → DeepSeek scoring → Sheets CRM | ✅ Defined |

---

## Ideated Agents (New — High-Pain Problems for Target Market)

| Agent | File | Purpose | Status |
|---|---|---|---|
| **Global Tech Stack Agent** | `global-tech-stack-agent.md` | US LLC + EIN + Stripe/Mercury + compliance for non-US founders | ✅ Defined |
| **Faceless Content Agent** | `faceless-content-agent.md` | Faceless short-form content: scripts, Bento Grid 3D, TTS, assembly | ✅ Defined |
| **Asian Market Entry Agent** | `asian-market-entry-agent.md` | Go-to-market blueprint for SEA/South Asia expansion | ✅ Defined |

---

## Agent Architecture (Common Pattern)

All agents follow **Observe → Think → Act** loop with:

- **Memory**: `mch-brain` (episodic + semantic) — tagged episodes for recall
- **Recon**: `mch-cyber/recon` — competitor, regulatory, market intelligence
- **Tools**: Domain-specific (n8n, content-engine, crucible, pxpipe, APIs)
- **Handoffs**: Explicit next-agent or human-in-the-loop gates

---

## Integration Map

```
User Idea
    │
    ▼
[Segway Test Agent] ──(Pothole)──→ Reframe / Abandon
    │
    └──(Well)──→ [Pain Extractor Agent]
                      │
                      ▼
              [Lead Gen Agent] ──→ Prospects → Sheets
                      │
                      ▼
              [Outreach Agent] (human-in-the-loop send)
                      │
                      ▼
              [Global Tech Stack Agent] ──→ Entity + Payments
                      │
                      ▼
              [Asian Market Entry Agent] ──→ Regional Expansion
                      │
                      ▼
              [Faceless Content Agent] ──→ Audience + Inbound
```

---

## Memory Tags (mch-brain)

| Tag | Agents | Purpose |
|---|---|---|
| `segway-test` | Segway Test | Idea evaluations, verdicts |
| `pain-extraction` | Pain Extractor | Pain dossiers, MVP specs |
| `lead-gen` | Lead Gen | Lead profiles, outreach logs |
| `outreach` | Lead Gen, Global Stack | Contact history, responses |
| `global-stack` | Global Tech Stack | Filings, deadlines, docs |
| `compliance` | Global Stack, Asia Entry | Regulatory deadlines, filings |
| `market-entry` | Asia Entry | Blueprints, checklists |
| `faceless-content` | Faceless Content | Content calendar, viral patterns |
| `viral-patterns` | Faceless Content | Hooks, structures, CTAs |

---

## Tool Dependencies

| Agent | mch-brain | mch-cyber | content-engine | crucible | pxpipe | n8n | APIs |
|---|---|---|---|---|---|---|---|
| Segway Test | ✅ | ✅ | | | | | Web Search |
| Pain Extractor | ✅ | ✅ | | | | | Web Search, Summarize |
| Lead Gen | ✅ | ✅ | | | | ✅ | Places, DeepSeek, Sheets |
| Global Tech Stack | ✅ | ✅ | | | | | Web Search, Summarize |
| Faceless Content | ✅ | | ✅ | ✅ | ✅ | | TTS, ffmpeg |
| Asia Market Entry | ✅ | ✅ | | | | | Web Search, Lead Gen |

---

## Next Steps

1. **Implement loops** as executable code (Python/Rust) in `crates/mch-agents/`
2. **Wire MCP tools** for content-engine, crucible, n8n
3. **Add pxpipe middleware** for token-efficient prompting
4. **Build Orchestrator** that chains agents per the integration map
5. **Add verification gates** (mch-gate) for each handoff