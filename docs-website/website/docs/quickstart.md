---
title: Quickstart
sidebar_label: Quickstart
sidebar_position: 2
slug: /quickstart
description: Install McHarness with one command and initialize the Key Farm.
---

# Quickstart

This guide takes you from nothing to a working McHarness in your terminal: install it, initialize the Key Farm, point it at a model, and run your first task that survives real use.

Pick the row that matches your goal:

| Goal | Do this first | Then do this |
|---|---|---|
| I just want McHarness running on my machine | `curl` install | Run `mcharness` and verify it responds |
| I already know which model I want | Initialize the Key Farm | Route a task to that model |
| I want rate-limit immunity across many keys | Add several keys to the Farm | Let the Farm fail over automatically |
| I want a private or local model | Add a local endpoint to the Farm | Verify the endpoint and model name |

**Rule of thumb:** if McHarness cannot complete a normal task, do not add more features yet. Get one clean run working first, then layer on the Forge, skills, and routing.

## Install

McHarness ships as a one-line installer that detects your operating system, fetches the right release binary, and puts the `mcharness` command on your `PATH` — the same experience as Hermes and OpenCode.

```bash
curl -fsSL https://get.mcharness.dev/install.sh | bash
```

:::note
The `get.mcharness.dev` installer URL is the target coordinate and is **not live yet** — the release pipeline is staged and will publish versioned archives for Linux, macOS, and Windows on first tag. Until then, build from source (below).
:::

After it finishes, reload your shell:

```bash
source ~/.bashrc   # or source ~/.zshrc
```

Verify the install:

```bash
mcharness --version
```

### Build from source

If you would rather build it yourself, McHarness is a Cargo workspace with C++ and Python modules:

```bash
git clone https://github.com/McOmokaro/McHarness.git
cd McHarness
cargo build --release
```

## Initialize the Key Farm

The **Key Farm** is the flagship differentiator: instead of juggling API keys by hand, you add them to one pool and McHarness routes across them automatically.

Initialize the Farm:

```bash
mcharness keyfarm init
```

Add one or more keys. Every key you add joins the shared pool:

```bash
mcharness keyfarm add --provider openai   --key sk-...
mcharness keyfarm add --provider deepseek --key sk-...
mcharness keyfarm add --provider anthropic --key sk-ant-...
```

Route a task to a model by name. The Farm finds **every** key that carries that model and lines them up as fallbacks:

```bash
mcharness keyfarm route --task design --model deepseek-chat
mcharness keyfarm route --task code   --model claude-sonnet
```

Inspect the Farm at any time. The live panel shows key health and which keys are approaching their rate limits:

```bash
mcharness keyfarm status
```

When a key nears its limit, the Farm **pauses and reroutes before a request fails** — rate limits stop being walls. If your chosen model is unavailable everywhere, the Farm consults the top model leaderboards for that task domain and selects the next best proven model automatically.

:::tip Local and private models
The Farm is fully model-agnostic. Add any OpenAI-compatible endpoint — Ollama, vLLM, a self-hosted gateway — and it behaves like any other provider:

```bash
mcharness keyfarm add --provider local --base-url http://127.0.0.1:11434 --key none
```
:::

## Run your first task

Start McHarness in your terminal:

```bash
mcharness
```

You will see the TUI with the live Key Farm panel, active model, and available skills. Give it a task that is specific and easy to verify:

```bash
Summarize this repository in 5 bullets and tell me the main entrypoint.
```

**What success looks like:**

- The TUI shows your chosen model and the routed key
- McHarness replies without error
- It completes the task end to end without a visible hand-off between tools
- You can run a second task in the same session and it still works

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `mcharness: command not found` | Shell not reloaded after install | `source ~/.bashrc` or open a new terminal |
| Empty or broken replies | No model routed, or a key is invalid | Run `mcharness keyfarm status` and confirm the task-to-model route |
| Requests fail with a 429 | Single key hit its rate limit | Add another key to the same provider — the Farm fails over automatically |
| Custom endpoint returns garbage | Wrong base URL or model name | Verify the endpoint in a separate client first |
| A tool is missing when invoked | Lazy capability not yet fetched | Trigger it once — it installs on first use (the Omarchy model) |

## Quick reference

| Command | Description |
|---|---|
| `mcharness` | Start McHarness in the terminal |
| `mcharness keyfarm init` | Initialize the Key Farm |
| `mcharness keyfarm add` | Add an API key to the shared pool |
| `mcharness keyfarm route` | Route a task to a model by name |
| `mcharness keyfarm status` | Show key health and rate-limit warnings |
| `mcharness --version` | Print the installed version |

## Where to go next

- **[Architecture](/architecture)** — the language stack and how the Whole is built
- **[The Forge](/the-forge)** — how McHarness's capabilities were reverse-engineered