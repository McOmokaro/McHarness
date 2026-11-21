---
name: lead-gen-agent
description: Autonomous lead generation system for web development targeting Asian SMEs. Uses Google Places API for discovery, Tilion cloud browser for bot-protected site enrichment, DeepSeek for AI scoring, and Google Sheets for CRM. Implements the "Asian Empire" playbook from Iman's framework.
capabilities:
  - mch-brain (semantic memory for lead profiles, episodic for outreach history)
  - mch-cyber (recon for tech stack detection, social signals)
  - tilion (cloud stealth browser — read_page / scrape_page / collect_data / solve_challenge)
  - n8n (workflow orchestration)
  - Google Places API, DeepSeek API, Google Sheets API
---

# Lead Gen Agent (Asian Empire)

**Source:** AI Income Challenge + `docs/lead-gen-system.md` — "Asian Empire" web dev playbook.

## Purpose

Find, score, and organize qualified web development prospects:
1. **Discover** — Google Places API: high-rated, review-rich local businesses **without websites**
2. **Enrich** — Tech stack detection, social presence, buying signals
3. **Score** — DeepSeek AI: 1-10 likelihood to buy (high reviews + no site = high score)
3. **Organize** — Google Sheets CRM with outreach-ready data
4. **Outreach** — Generate personalized first contact (human-approved)

## Loop: Observe → Think → Act

### Observe (Daily/Scheduled)
- Trigger: n8n cron (daily 06:00 local) or manual
- Query: Google Places Text Search — `category + city` pairs from ICP config
- Pull from `mch-brain`: previous lead scores, outreach history, conversion patterns
- Pull from `mch-cyber/recon`: Tech stack fingerprint (Wappalyzer-style), social activity

### Think (Per Lead)
- **Buying Signal Check**: No website? Broken site? Only Facebook/IG? Active social but no web?
- **AI Score** (DeepSeek `deepseek-flash`): 1-10 + pain point + outreach angle
- **Threshold Filter**: Score ≥ 8 → qualified
- **Dedup**: By `place_id` — update existing, don't duplicate

### Act
- **Append/Update** Google Sheets row (10 columns)
- **Generate** personalized outreach draft (DeepSeek)
- **Queue** for human review/approval
- **Log** episode in `mch-brain` (tagged `lead-gen`, `outreach`)

## Tool Access

| Tool | Purpose |
|---|---|
| `mch-brain::recall` | Lead history, conversion patterns, industry benchmarks |
| `mch-brain::store` | Lead profiles, outreach logs, outcomes |
| `mch-cyber::recon` | Tech stack detection, SSL cert, social footprint |
| `tilion.read_page` | **Replaces old scrape/probe** — read a prospect's site (or social page) as clean Markdown, with Cloudflare/DataDome/CAPTCHA recovery |
| `tilion.scrape_page` | One-off page read, no session (cheap single-URL enrichment) |
| `tilion.collect_data` | Pull structured records (reviews, contact info, social links) from a page in plain English |
| `tilion.solve_challenge` | Clear a CAPTCHA / bot check when a page is blocked |
| `n8n_workflow` | Execute the Places → Score → Sheets pipeline |
| `deepseek_score` | AI scoring call (internal) |
| `google_sheets` | CRM read/write |

## n8n Workflow (Embedded)

```
Schedule → Places Search → Batch Split → Place Details → 
Signal Rules → Tilion read_page (site probe + block recovery) → DeepSeek Score → Filter (score≥8) → 
Dedup → Google Sheets Append → Outreach Draft Queue
```

## Tilion Integration (replaces legacy scraping)

The old HTTP-probe / scraper node is replaced by the **Tilion cloud stealth browser** (MCP server `tilion`, wired in `~/.hermes/config.yaml`). It recovers pages that block ordinary automation (Cloudflare, DataDome, CAPTCHAs, 403 walls) and reports `content_ok` / `status` on every read.

| Step | Tilion call | Notes |
|---|---|---|
| Probe prospect website | `read_page {url, session_id}` | Returns clean Markdown + `status` (`ok`/`blocked`/`empty`). `empty`/dead site = strong buying signal. |
| Single-URL enrichment | `scrape_page {url}` | No session; cheapest for one-off reads. |
| Pull reviews / contacts / socials | `collect_data {url, prompt}` | Structured records in plain English. |
| Page is bot-blocked | `solve_challenge {session_id}` then re-`read_page` | Clears CAPTCHA / bot check. |
| Session hygiene | `create_browser_session` … `close_browser_session` | **Always close** — billing is per-minute while open. Reuse one session per batch. |

**Verified live (2026-11-21):** `create_browser_session → read_page(https://www.apple.com/mac/) → close_browser_session` returned `status: ok`, title "Mac - Apple", 3078 chars Markdown; cost ≈ 0.02 credits. Account `polymorphhq@gmail.com`, free tier, 2500 credits.

> Tool-name note: Tilion has **no** `fetch_protected_page` / `extract_page`. The real equivalents are `read_page`, `scrape_page`, `collect_data` (+ `solve_challenge`).

## ICP Config (Asian Empire)

| Market | Categories | Cities (Tier-2) |
|---|---|---|
| Indonesia | roofing, plumbing, dental, salon, restaurant | Surabaya, Bandung, Medan |
| Philippines | construction, clinic, spa, restaurant | Cebu, Davao, Baguio |
| Vietnam | renovation, dental, gym, cafe | Da Nang, Can Tho, Hue |
| Malaysia | contractor, clinic, tuition, F&B | Ipoh, JB, Kota Kinabalu |

## Output Schema (Google Sheets Row)

| Company | Category | City | Phone | Website | Has Site | Rating | Reviews | AI Score | Pain Point | Outreach Angle | Status | First Seen |
|---|---|---|---|---|---|---|---|---|---|---|---|---|

## Cost Model

| Component | Monthly Cost |
|---|---|
| Google Places (Essentials tier) | $275 (100k calls) |
| DeepSeek scoring | ~$0.05 (negligible) |
| n8n (self-hosted) | $0 |
| **Total** | **~$275/mo** (after $300 trial credit) |

## Integration

- Invoked by: Orchestrator (scheduled) or User (manual)
- Hands off to: Outreach Agent (human-in-the-loop send)
- Memory: `mch-brain` episodes tagged `lead-gen`, `outreach`
- Dossier: `docs/lead-gen-system.md`