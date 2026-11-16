# Social Media Scraping APIs — Curated API Collection

**Repo:** `cporter202/social-media-scraping-apis` (2.8k⭐, JavaScript) — active (pushed Oct 20)
**Forge path:** `~/McHarness/forge/social-media-scraping-apis`
**License:** Not specified (README is marketing-heavy)

---

## What it is

**3,268 ready-to-use APIs** for scraping/extracting data from social media platforms. A curated index, not a unified SDK.

Platforms covered: **Instagram, LinkedIn, X/Twitter, TikTok, YouTube, Facebook**, and more.

**Data types:** posts, profiles, videos, comments, engagement metrics, followers, hashtags, stories, reels.

---

## Structure

```
social-media-scraping-apis/
├── README.md                 # 1.3MB — massive marketing page + API tables
├── social-media-apis-3268/   # Core data (likely JSON/CSV)
├── settings/                 # Config / metadata
└── .github/                  # Workflows
```

The README is a **single massive HTML/Markdown page** with embedded tables, badges, and marketing. The actual API data lives in `social-media-apis-3268/` (needs inspection).

---

## API Categories (from README)

| Platform | Example Endpoints |
|---|---|
| **Instagram** | Profile info, posts, reels, stories, highlights, followers, hashtag search |
| **LinkedIn** | Profile, posts, company pages, job postings, sales navigator data |
| **X/Twitter** | Tweets, user timeline, search, spaces, lists, bookmarks |
| **TikTok** | User profile, videos, comments, trending, hashtag, music |
| **YouTube** | Channel, videos, comments, playlists, shorts, transcripts |
| **Facebook** | Pages, groups, posts, events, marketplace |

---

## McHarness Integration Plan

| McHarness Crate | Use Case |
|---|---|
| `mch-cyber` / `mch-scope` | OSINT enrichment — social footprint of targets |
| `mch-trade` | Sentiment / signal extraction from financial twitter/X |
| `mch-hack` | Hackathon / bounty target reconnaissance |
| `mch-brain` | Episodic memory: ingest social signals as episodes |
| **Lead-gen (n8n)** | **Enrichment node** — after Places API finds a business, query their social presence (active? dead? engaged?) → buying signal |

**Integration steps:**
1. Parse `social-media-apis-3268/` — determine format (JSON? CSV? API specs?)
2. Build a **unified adapter layer** in `mch-mcp` or `mch-scope`: normalize auth, rate limits, pagination across providers
3. Add as **n8n custom nodes** (or HTTP Request templates) for the lead-gen workflow
4. Cache responses in `mch-brain` (semantic memory) with TTL
5. **Critical:** Most of these are *unofficial/scraper APIs* — ToS risk, rate limits, instability. Tag each with `risk: high|medium|low`, `official: boolean`.

---

## Risk Assessment

| Factor | Assessment |
|---|---|
| **Legality** | Scraping social platforms often violates ToS; some jurisdictions (EU, CA) have stronger protections |
| **Stability** | Unofficial APIs break when platforms change HTML/GraphQL |
| **Auth** | Many require cookies, tokens, or residential proxies |
| **Cost** | Some are paid SaaS with per-call pricing |
| **Ethical** | Personal data extraction → GDPR/CCPA/PDPA compliance required |

**Recommendation:** Use only **official APIs** (Graph API, Twitter API v2, YouTube Data API, LinkedIn Marketing API) for production. Treat this repo as a **discovery index** — verify each endpoint's legitimacy before wiring.

---

## Dossier Status

`ingested` — ready for adapter layer. **Lead-gen workflow should use official APIs first; this repo is a fallback/map.**