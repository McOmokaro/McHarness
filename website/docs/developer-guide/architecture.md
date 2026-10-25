---
id: architecture
title: Architecture
slug: /docs/developer-guide/architecture
sidebar_label: Architecture
pagination:
  prev: /docs/learning-path
---
# Architecture

How McHarness works under the hood.

## Core Components

### The Learning Loop

McHarness's defining feature is its closed learning loop:

1. **Experience** - Agent interacts with tools and environment
2. **Skill Creation** - Automatic procedural skill generation
3. **Skill Self-Improvement** - Honcho dialectic refinement during use
4. **Nudge & Persist** - Periodic memory consolidation across sessions
5. **User Modeling** - Growing understanding of user preferences

### Tool Gateway

60+ tools unified behind a single gateway, configurable per provider:
- Web search, Image generation, TTS, Browser
- Terminal backends (Docker, SSH, Daytona, Modal)
- Messaging platforms (20+ supported)

### Memory System

- FTS5 vector search for cross-session recall
- LLM summarization of trajectory data
- Honcho dialectic user modeling
- Periodic nudges to persist important information

### Bot Mode

- Named bots with independent model/configuration
- Skill libraries per bot
- Group chat coordination via `@mentions`

## Extending McHarness

### Adding New Tools

1. Define tool in `config.yaml`
2. Implement executor in `src/tools/`
3. Register in the gateway middleware

### Creating Skills

Skills are auto-created but can be manually defined:
```yaml
skills:
  - name: "file-search"
    description: "Search files by content"
    trigger: "when user asks to find files"
    executor: "file_tool.search"
```

<Search />