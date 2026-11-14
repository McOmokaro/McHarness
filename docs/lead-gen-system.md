# Asian Empire Lead-Gen System (n8n)

**Status:** design spec — grounded draft, not yet built (no n8n install, no Google/DeepSeek keys on the VPS yet).
**Source idea:** `Miracle Notes/Business!/IDea for Web dev.md` (@marlowxbt viral play) — local SMBs with strong Google reviews and **no website** are the buying signal; sell them a site.

---

## 0. Honest framing (read first)

The viral story scrapes Google Maps (e.g. via a scraper). **Google Maps Platform Terms forbid scraping/caching the Maps data** — that path gets your key banned and is legally shaky. The durable, ToS-clean path is the **official Places API**. Everything below uses the official API. Scraping the HTML `google.com/maps` page is a *short-term hack*, not a system.

Also: cold-email/outreach has real legal constraints (CAN-SPAM in the US, GDPR/PECR in the EU, and most APAC markets have their own rules). "Send 500 emails/day, 3% reply" is fine as a *volume hypothesis*; treat consent, unsubscribe, and a real sender identity as hard requirements, not afterthoughts.

---

## 1. Ideal Customer Profile (ICP)

| Attribute | Target |
|---|---|
| Business type | Local service SMB — roofing, plumbing, HVAC, landscaping, dental, salons, restaurants, clinics |
| Market | Asia-first per the play (e.g. Indonesia, Philippines, Vietnam, India, Malaysia, Thailand tier-2 cities) |
| Review signal | ≥ 4.5 stars, ≥ 15 reviews (proven demand) |
| **Pain signal** | **No website** in the Places record, OR only a Facebook/Instagram page (→ `signal_socialOnly`, see §3b), OR a dead/last-updated-years-ago site |
| Reachability | Has a public phone/email |
| Budget proxy | Established enough to pay (multi-year, multiple reviews) |

The single strongest signal remains **"great reputation + zero or broken web presence."**

---

## 2. Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│ n8n (self-hosted, Docker on the VPS)                                   │
│                                                                        │
│  [1] Schedule / Manual trigger                                         │
│        │                                                               │
│  [2] Places API — Text Search / Nearby Search   (discover businesses)  │
│        │                                                               │
│  [3] Split In Batches (rate-limit friendly)                            │
│        │                                                               │
│  [4] Places API — Place Details                  (website, phone,     │
│        │                                          rating, reviews)      │
│  [5] Buying-signal rules (code node)             (no/broken website?)  │
│        │                                                               │
│  [6] Website probe (optional)  HEAD/GET homepage → 4xx/5xx/timeout=     │
│        │                       "broken"; 200=has site                   │
│  [7] AI Scoring (DeepSeek `deepseek-flash`)       (1–10 + pain point)  │
│        │                                                               │
│  [8] Dedup + filter (score ≥ threshold)                                │
│        │                                                               │
│  [9] Append/Update → Google Sheets (qualified prospects)                │
│        │                                                               │
│ [10] (later) Outreach queue → email/LinkedIn draft (human-approved)     │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 3. Data sources & APIs

| Layer | API / tool | Notes |
|---|---|---|
| Business discovery | **Google Places API — Text Search / Nearby Search** | Query e.g. `"plumber in Cebu"`. Returns name, place_id, rating, review count. |
| Business detail | **Google Places API — Place Details** | `website`, `formatted_phone_number`, `rating`, `user_ratings_total`, `url`, address. Website field is what drives the whole signal. |
| Website probe | n8n **HTTP Request** node | GET the `website` URL; 200 = live, 4xx/5xx/timeout = broken. |
| Scoring | **DeepSeek API** (`deepseek-flash`) | Cheap JSON-mode scoring call per lead. Model name is `deepseek-flash`. |
| Storage | **Google Sheets API** (n8n Google Sheets node) | Output sheet. |
| Scheduling | n8n **Cron/Schedule** node | Daily/weekly runs. |
| (Social-signal enrichment) | `cporter202/social-media-scraping-apis` (repo in forge) | Catalog of 3,268 scrapers (Instagram/Facebook/LinkedIn/TikTok/WhatsApp…). Used for the active-social / no-website signal — see §3b. |

---

## 3b. Social-signal buying signals (via `social-media-scraping-apis`)

**What the repo actually is:** a *catalog*, not runnable code — `forge/social-media-scraping-apis/social-media-apis-3268/README.md` is a table of **3,268** scraper APIs (mostly [Apify](https://apify.com) actors, plus apimesa endpoints), each with a name, description, and link. We pick from it; we don't call the repo. Treat each entry as a **third-party, paid, ToS-bound API** (Apify actors bill per run/per-1k results), not a free dependency.

**Why it matters — the second-best buying signal.** Places tells us who has *no website* (§1). This catalog lets us tell who has **an active social presence but no website** — the strongest "they already invest in marketing, they just haven't bought a site" prospect for the Asian Empire play.

The classic signal pattern: *Instagram / Facebook page active, website field empty.* That business is paying attention to customers online, converting some, and losing the rest to the missing site — a warm, specific pitch ("your IG is great; customers searching you find nothing to land on").

**How we wire it (enrichment stage, after Node 5):**

1. **Confirm a social asset exists.** Use a social *search/scraper* entry to look up `displayName` + city. Useful catalog rows:
   - Instagram/Facebook page lookup & post metadata (e.g. the `apimesa` "Instagram Public Post Details" / "Facebook …" entries; Apify "Instagram Post & Video Scraper" family) → does an active page exist, and how recent are the last posts?
   - LinkedIn company lookup (Apify "LinkedIn Company URL Bulk Profile Finder") → does the business have a registered company page (legitimacy proxy)?
2. **Confirm reachability.** WhatsApp validator entry (Apify "WhatsApp Number Validator") → is the Places phone a live WhatsApp number (the dominant outreach channel in the target APAC markets)?
3. **Score the combined signal.** Extend Node 5:

```js
const hasSite    = !!(place.websiteUri && place.websiteUri.length);
const hasSocial  = !!(social?.found && social.last_post_days <= 90); // active in last quarter
const whatsapp   = !!reach.whatsapp_registered;
return {
  ...place,
  signal_noWebsite:   !hasSite,
  signal_socialOnly:  !hasSite && hasSocial,   // ⭐ the gold signal
  signal_liveWhatsApp: whatsapp,
};
```

4. **Feed the score.** A `signal_socialOnly` lead gets a floor bump in the §4 Node-7 scoring prompt (strong reviews + active social + no website ⇒ near-max), and an outreach angle of the form *"saw your Instagram — customers who Google you find nothing."*

**Cost & caveats (same discipline as §6):**

- Apify actors are **metered** (commonly ~$2–$30 per 1,000 results depending on actor/platform); budget per enrichment call and cap fan-out. Do the cheap Places pass first, enrich only leads that already pass §5.
- These are **third-party scrapers**: reliability varies, ToS and platform anti-bot rules apply, and SLA is the vendor's, not ours. Never make the **spine** depend on a scraper — it's optional enrichment that can fail soft (a missing social signal degrades a lead's score, it doesn't break the run).
- Same outreach-compliance rules as §0: confirming a social/WhatsApp presence is data collection, not consent to contact.

---

## 4. Workflow steps (node by node)

**Node 1 — Trigger.** Schedule (e.g. daily 06:00 local) or Manual, so the first runs are hand-supervised.

**Node 2 — Discover (HTTP Request → Places Text Search).**
`GET https://places.googleapis.com/v1/places:searchText`
Headers: `X-Goog-Api-Key`, `X-Goog-FieldMask: places.id,places.displayName,places.rating,places.userRatingCount`
Body: `{ "textQuery": "plumber in Cebu" }`
Iterate city × category pairs from a config array.

**Node 3 — Split In Batches (size 1–10).** Keeps you inside Places QPS and makes retries clean.

**Node 4 — Details (HTTP Request → Places Place Details).**
`GET https://places.googleapis.com/v1/places/{place_id}`
`X-Goog-FieldMask: id,displayName,rating,userRatingCount,websiteUri,nationalPhoneNumber,formattedAddress,businessStatus`
→ `websiteUri` empty = the golden signal.

**Node 5 — Buying-signal rules (Code node).** Produce booleans:
```js
const hasSite = !!(place.websiteUri && place.websiteUri.length);
return { ...place, signal_noWebsite: !hasSite, signal_lowReviews: place.userRatingCount < 15 };
```

**Node 6 — Website probe (optional, HTTP Request).** If `hasSite`, GET `websiteUri` with a short timeout; tag `site_broken` on 4xx/5xx/timeout. Skip if no site.

**Node 7 — AI Scoring (HTTP Request → DeepSeek).** `POST https://api.deepseek.com/chat/completions`, `model: "deepseek-flash"`, `response_format: {type:"json_object"}`. Prompt:

> You score local businesses 1–10 on likelihood to buy a new or replacement website. Output strict JSON: `{"score": <int 1-10>, "pain_point": "<short reason>", "outreach_angle": "<one line>"}`.
> Inputs: name={}, category={}, rating={}, review_count={}, has_website={}, site_broken={}.
> High score = strong reviews AND (no website OR broken/outdated site). Low score = good reviews AND a modern active site. Be conservative; do not reward unknown data.

**Node 8 — Filter + dedup (Code/Remove Duplicates).** Keep `score >= threshold` (start at 8). Dedup on `place.id`.

**Node 9 — Google Sheets Append.** Columns below. Use `place.id` as the dedup key so re-runs update rather than duplicate.

**Node 10 — Outreach (later phase).** Generate a personalized first line per lead (DeepSeek), drop into a `Draft` column. **Human approves before any send.**

---

## 5. Output sheet schema

| Company Name | Category | City | Phone | Website | Has Site | Rating | Reviews | AI Score | Pain Point | Outreach Angle | Status | First Seen |
|---|---|---|---|---|---|---|---|---|---|---|---|---|

---

## 6. Estimated costs (grounded, verify before scaling)

**Google Maps Platform** now sells subscription tiers plus pay-as-you-go; new customers get a **$300 trial credit**. Published tiers: **Starter $100/mo (50k calls)**, **Essentials $275/mo (100k calls)**, **Pro $1,200/mo (250k calls)**.
- Discovery + details are the metered calls. A run of **2,000 businesses** ≈ 2k Text Search + 2k Place Details = ~4k calls. At the Essentials tier (100k calls) you can do ~25 such runs/month.
- **Realistic early cost: $0 during the $300 trial, then $100–$275/mo** once you pass the free credit at real volume. (The old "free $200/mo forever" era is over — pricing changed to subscription tiers.)

**DeepSeek** (`deepseek-flash`, per 1M tokens; off-peak = half price):
- Scoring ~2,000 leads: each call is tiny (~300 in / ~60 out tokens) ⇒ **roughly a few cents total**. Negligible.
- Bonus first-party tools if you use them: Search/Fetch **$0.75 per 1,000 calls**; OCR/file parsing **$0.225 per 1,000 pages**; TTS $0.225/10k chars; ASR $0.058/hr (limited-time 50% off).

**n8n** — self-hosted on the VPS = **$0** software (Docker). Only infra is the VPS you already run.

**Monthly all-in once past trial:** ~**$100–$300** dominated by Google Places; DeepSeek is pennies. The "$10/month" figure in the playbook is not achievable with the official API at any real volume; it only holds if you scrape (to which the ToS objection applies).

---

## 7. Build order (when we execute)

1. Enable the **Places API** in a Google Cloud project, mint a key, restrict it to Places API + server IP.
2. `docker run n8n` on the VPS; add Google + DeepSeek credentials.
3. Build Nodes 1→5; run **one** city/category; eyeball output.
4. Add Node 7 scoring; tune the threshold on 50 real leads.
5. Wire Sheets (Node 9); schedule.
6. Only then design the outreach node — with human approval and a compliant sender.

---

## 8. Open questions / blockers

- **ICP markets:** "Asian empire" needs concrete city×category lists before discovery can run. Not yet defined in the repo.
- **Email source:** Places does not return emails; a compliant enrichment step is required for outreach (separate decision).
- **Legal review:** outreach compliance per target country before any send.
