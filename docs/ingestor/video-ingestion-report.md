# Video Ingestion Status Report

**Date:** 2026-11-16
**Phase:** Deep Ingestion and Agent Forging

---

## Phase 1: Summarize Tool Installation ✅

- `npm i -g @steipete/summarize` — **success** (v1.0.0 installed)
- Tool functional for web page extraction (tested on example.com)
- Configured for OpenRouter + DeepSeek (requires `OPENROUTER_API_KEY` env var)

---

## Phase 2: Summarize Skill ✅

- Updated `~/.hermes/skills/summarize.md` with full specification
- Defines `summarize_url` tool, YouTube modes, output to `~/McHarness/docs/ingestor/`
- Integrated with content-engine, dsh-deepread

---

## Phase 3: Video Ingestion — **BLOCKED** ❌

### Attempted URLs (8 provided)

| # | URL | Type | Result |
|---|---|---|---|
| 1 | `https://youtube.com/playlist?list=PLEBxy457FACw` | Main Sessions Playlist | Returns "- YouTube" only |
| 2 | `https://youtube.com/playlist?list=PLecUmotwpUZ4` | VIP Playlist 1 | Returns "- YouTube" only |
| 3 | `https://youtube.com/playlist?list=PLecUmotwpUZ4` | VIP Playlist 2 (duplicate) | Returns "- YouTube" only |
| 4 | `https://www.youtube.com/live/vtKJgp_9Zbk` | Live Session | Requires transcription provider |
| 5 | `https://www.youtube.com/live/QOtiU4pkd-E` | Live Session | Requires transcription provider |
| 6 | `https://www.youtube.com/live/N0yLwV4F4Ns` | Live Session | Requires transcription provider |
| 7 | `https://www.youtube.com/live/IembGIN63h0` | Live Session | Requires transcription provider |
| 8 | `https://www.youtube.com/live/EAZnoK5Xr24` | Live Session | Requires transcription provider |

### Root Cause: YouTube Authentication Required

**Both `summarize` and `yt-dlp` (used by content-engine) require valid YouTube authentication:**

1. **`summarize --youtube yt-dlp`** → "Missing transcription provider (install whisper-cpp or set GROQ_API_KEY/ASSEMBLYAI_API_KEY/GEMINI_API_KEY/OPENAI_API_KEY/FAL_KEY/DEEPGRAM_API_KEY)"
2. **`summarize --youtube web`** → Returns only "- YouTube" (no transcript extraction)
3. **`yt-dlp --write-auto-subs`** → "Sign in to confirm you're not a bot. Use --cookies-from-browser or --cookies"
4. **`content-engine ce_fetch_transcript`** → Same yt-dlp auth failure

### User's cookies.txt Status

```bash
~/run_vip_download.sh uses: --cookies /home/ubuntu/cookies.txt
```
**Result:** Stale cookies → 403/429 bot detection

### Required to Unblock

| Option | What's Needed |
|---|---|
| **Fresh YouTube cookies** | Export from authenticated browser session → `/home/ubuntu/cookies.txt` |
| **Transcription API key** | `GROQ_API_KEY` or `ASSEMBLYAI_API_KEY` or `GEMINI_API_KEY` for `summarize --youtube yt-dlp` |
| **OpenRouter + DeepSeek key** | `OPENROUTER_API_KEY` for AI summarization (user said they'd load this) |

---

## Phase 4: Specialized Agents Built ✅

**Location:** `~/McHarness/src/agents/`

| Agent | Source | File |
|---|---|---|
| **Segway Test Agent** | Iman's AI Income Challenge | `segway-test-agent.md` |
| **Pain Extractor Agent** | Iman's AI Income Challenge | `pain-extractor-agent.md` |
| **Lead Gen Agent** | Iman's AI Income Challenge + `docs/lead-gen-system.md` | `lead-gen-agent.md` |

### Common Architecture
- **Loop**: Observe → Think → Act
- **Memory**: `mch-brain` (episodic + semantic, tagged)
- **Recon**: `mch-cyber/recon` for market/competitive intelligence
- **Handoffs**: Explicit next-agent gates

---

## Phase 5: New Agents Ideated & Built ✅

| Agent | Target Pain Point | File |
|---|---|---|
| **Global Tech Stack Agent** | Non-US founders: "I have customers but can't accept US payments" — US LLC, EIN, Stripe/Mercury, BOI, compliance | `global-tech-stack-agent.md` |
| **Faceless Content Agent** | "Build audience without showing face" — content-engine clips + Crucible Bento Grid 3D + pxpipe token compression | `faceless-content-agent.md` |
| **Asian Market Entry Agent** | "Asian Empire" expansion — SEA/South Asia go-to-market: entity, compliance, payments, hiring, channels | `asian-market-entry-agent.md` |

### Integration Map
```
User Idea → Segway Test → Pain Extractor → Lead Gen → Outreach
                                    ↓
                              Global Tech Stack → Entity + Payments
                                    ↓
                              Asian Market Entry → Regional Expansion
                                    ↓
                              Faceless Content → Inbound Audience
```

---

## Phase 6: Commit & Push ✅

**Commits:**
1. `01ae31c` — lead-gen-system.md + social-media-scraping-apis.md
2. `2a0da87` — 6 agent definitions + README in `src/agents/`

**Pushed to:** `origin/main` (GitHub: McOmokaro/McHarness)

---

## Next Actions Required (User)

1. **Provide fresh YouTube cookies** → Update `/home/ubuntu/cookies.txt`
2. **Set `OPENROUTER_API_KEY`** with DeepSeek access for AI summarization
3. **Optionally: Set transcription API key** (`GROQ_API_KEY` etc.) for `summarize --youtube yt-dlp`
4. **Re-run video ingestion** once auth is configured
5. **Implement agent loops** as executable code in `crates/mch-agents/`
6. **Wire MCP tools** for content-engine, crucible, n8n
7. **Add pxpipe middleware** for token-efficient prompting