---
sidebar_position: 1
slug: /
---
# McHarness

**Father of Harnesses — one whole, light-speed, featherweight multi-agent harness.**

McHarness is a next-generation multi-agent orchestration framework designed for simplicity, reliability, and extreme performance. Built on the principles of lightweight agent coordination and deterministic task execution, McHarness enables developers to compose complex agent workflows with minimal configuration and maximum observability.

## Core Philosophy

- **Featherweight**: No bloat. Zero-runtime overhead for simple compositions.
- **Observable**: Built-in tracing and metrics for every agent interaction.
- **Composable**: Chain agents, fans out, or nest workflows recursively.
- **Secure**: Sandboxed execution contexts with capability-based access control.

## Quick Start

```bash
# Install
npm install mchardness

# Define an agent
export const research_agent = {
  name: 'researcher',
  instructions: 'Find the best resources on the given topic',
  tools: ['web_search', 'read_article'],
};

# Run a workflow
mc-harness run research_agent --topic "AI agents";
```

## Documentation

- [Roadmap](/roadmap) — planned features and milestones
- [REA Integration](/rea-integration) — Apple design token pipeline