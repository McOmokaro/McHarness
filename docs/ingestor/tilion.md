# Tilion — Cloud Stealth Browser for AI Agents

**Service:** https://tilion.com · API `https://back.tilion.dev` · MCP `https://back.tilion.dev/mcp`
**Account:** `polymorphhq@gmail.com` · free tier · 2500 credits ($5) · set up 2026-11-21
**Status:** `connected` — key saved `~/.config/tilion/credentials.json` (0600), MCP wired in `~/.hermes/config.yaml`, Hermes skill `tilion` installed.

---

## What it is

Tilion runs **real (stealth, not headless) Chrome sessions in the cloud** behind a REST API and an MCP server. It **recovers pages that block ordinary automation** — Cloudflare, DataDome, CAPTCHAs, 403 walls — and every response reports whether usable content came back (`content_ok` / `status`). This is the replacement for the forge's fragile scrape/probe logic and the answer to the YouTube/scraper bot-walls.

## Setup (chat sign-in, completed)

1. `POST /v1/agent/signin/start {agent:"Hermes", email}` → emailed 6-digit code + `device_code`.
2. Person pasted code → `POST /v1/agent/signin/verify {device_code, code}` → `api_key` (`ph_live_…`).
3. Saved key to `~/.config/tilion/credentials.json` (mode 600). Free check `GET /v1/usage` → `200`.
4. MCP wired into `~/.hermes/config.yaml` (`mcp_servers.tilion`, HTTP + bearer header). Tools load as `mcp_tilion_*` on next Hermes restart.

## Tool surface (48 MCP tools — real names)

> There is **no** `fetch_protected_page` / `extract_page`. Use the real names below.

| Need | Tool | Session? |
|---|---|---|
| Start browser (billing begins) | `create_browser_session` → `session_id`, `live_view_url` | yes |
| Page as clean Markdown + block recovery | `read_page {url, session_id}` | yes |
| One page, no session (cheapest) | `scrape_page {url}` | no |
| Structured records from a page | `collect_data {url, prompt}` | yes |
| Actionable elements → selectors | `get_elements` | yes |
| Interact | `click` / `fill` / `type_text` / `press_key` / `scroll` | yes |
| Clear CAPTCHA / bot check | `solve_challenge` · new IP: `change_exit` | yes |
| Sign-ins (person enters creds on Tilion page) | `login` / `credential_connect` / `save_login` | yes |
| Playwright / raw CDP | `get_cdp_endpoint` | yes |
| Crawl up to 100 pages | `crawl_site {url, max_pages}` | yes |
| **Stop billing** | `close_browser_session {session_id}` | — |
| Free health check | `profile_list` / `account_status` | no |

REST equivalents: `POST /v1/session`, `/v1/extract`, `/v1/fetch`, `/v1/scrape`, `/v1/search`, `DELETE /v1/session/{id}`.

## Billing

- Setup / sign-in / `GET /v1/usage` = **free**.
- `POST /v1/session`: ~2 credits create + 1 credit/min while open + 1 credit/action. `DELETE` = free, stops metering.
- Current rates: `GET /v1/credits`. Concurrency limit 3.

## Live test (2026-11-21)

`create_browser_session → read_page(https://www.apple.com/mac/) → close_browser_session`
→ `status: ok`, title "Mac - Apple", **3078 chars Markdown**, cost **≈ 0.02 credits**, session closed. Balance 2500 → 2499.98.

## Forge mapping

| McHarness | Tilion role |
|---|---|
| `mch-browser` | Cloud stealth backend option alongside Kitesurf/Chromium — use for bot-defended targets |
| `lead-gen-agent` | **Replaces website-probe/scraper node** with `read_page`/`collect_data` (see `src/agents/lead-gen-agent.md`) |
| `mch-cyber` / recon | Read protected competitor/market pages that block ordinary automation |
| Video ingestion | Candidate path for bot-walled transcript/source pages (YouTube still needs its own auth) |

## Hygiene rules

- Always `close_browser_session` (trap/finally). Reuse one session per batch.
- Trust `content_ok`/`status`, not HTTP code. Hard pages → `async_mode:true`, poll `/v1/job/{id}`.
- Never paste passwords in chat — `credential_connect` link. Ask before purchase/post/delete.
- Key stays out of messages/notes/committed files.