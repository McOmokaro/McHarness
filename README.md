# 🧠 McHarness (Father of Harnesses)

> **Status:** 🚧 Week 1 - Laying the Groundwork (Alpha) 🚧

**McHarness** is a sovereign, portable AI orchestration workspace designed to reverse-engineer, unify, and rebuild agentic workflows. It acts as a "father of harnesses," giving developers ultimate control over how local and cloud AI agents collaborate, execute tools, and manage memory.

Built with a focus on **portability**, the entire ecosystem (Python env, configs, skills, and data) can be packed into a single folder or USB drive and moved between machines.

## 🛠️ The Tech Stack
McHarness is being built from the ground up using high-performance languages:
*   **Python:** For rapid prototyping, AI bridges, and MCP server management.
*   **Rust:** For the core engine, speed, and memory safety.
*   **C++:** For deep system-level binary analysis and reverse engineering integrations.

## 🚀 Current Progress (Week 1)
We are currently in the architecture and setup phase. The current repository includes:
- [x] The Portable Workspace architecture (`start.sh`).
- [x] Integration of **OpenCode** as the primary agentic coder.
- [x] Integration of **REA** (Reverse Engineer Anything) for deep binary/web analysis.
- [ ] *Coming in Week 2:* The core Python orchestration loop.

## 💻 How to Run (For Terra Evaluators)
*Note: The core agent is currently under active development and will not run a full task yet.*

To initialize the portable environment:
```bash
chmod +x start.sh
./start.sh
