<div align="center">

# McHarness

**The father of harnesses.**

Built the Wozniak way: build the thing you want to use, from parts you understand, and nothing you don't. McHarness is the terminal agent I wanted — so it gets built in the open.

*One whole. Light speed. Featherweight.*

</div>

---

McHarness is a native multi-agent terminal harness forged from first principles. We are building it entirely from scratch to automate complex workflows, reverse-engineer closed-source software, and serve as a sovereign, portable workspace. Intent in, action out.

## The Tech Stack

We are building this system from the ground up using three core languages, each chosen for its specific engineering strengths:

**Rust (The Core Foundation)**
- **Agent Loop:** The heartbeat of the system. It is the continuous cognitive cycle (Observe → Think → Act) that allows the agent to process inputs, evaluate its state, and execute tools without hallucination.
- **Memory:** The persistent context engine. It handles the short-term working memory (chat history) and long-term episodic memory (vector databases) so the agent remembers what it learned yesterday.
- **Key Farm:** The dynamic API management layer. It maintains a live pool of provider keys, monitors rate limits, and seamlessly reroutes requests before a failure occurs.
- **Orchestration:** The multi-agent router. It manages how sub-agents spawn, communicate, and delegate tasks to one another without colliding.

**Python (The Research & Bridge Layer)**
- **Research Tooling:** The scripting environment for web scraping, data analysis, and formatting.
- **Data Ingestion:** The pipeline that parses massive external datasets (like Fabric ingestion) and extracts structured knowledge.
- **Bridge to Core:** The communication layer that allows Python scripts to send complex data structures directly into the Rust binary.

**C++ (The Speed Organs)**
- **Token Streaming:** Ultra-low-latency parsing of LLM responses to ensure the terminal UI prints words the millisecond they are generated.
- **Memory Search:** High-performance vector similarity search, allowing the agent to scan millions of past memories in milliseconds.
- **Embedding Math:** Fast matrix multiplication required for converting text into mathematical vectors.

## Roadmap

### 🟢 DONE
- [x] **Portable Workspace Layout:** Initialized the strict folder architecture (`core/`, `crates/`, `cpp/`, `python/`) designed for sovereign, offline-capable development.
- [x] **Foundation Setup:** Configured isolated local environments and strict dependency management.
- [x] **Core Crate Architecture:** Scaffolded the primary Rust modules (mch-core, mch-gate, mch-ledger, mch-scope, mch-audit).
- [x] **Repository Hygiene:** Enforced strict `.gitignore` rules, unified the README, and cleaned the Git index.
- [x] **License Protection:** Applied a Custom Proprietary Evaluation License to protect the intellectual property.

### 🟠 IN PROGRESS
- [x] **Core Rust Organs:** Building the agent loop, memory modules, and the Key Farm setup.
- [ ] **C++ Speed Organs:** Writing the token streaming and high-speed memory search logic.
- [ ] **Python Bridge:** Writing `python/bridge.py` to connect the research tooling to the Rust core.

### 🔴 PLANNED
- [ ] **Key Farm (Full Implementation):** Unifying API keys into a single pool with live rate telemetry and reroute-before-failure logic.
- [ ] **Domain Leaderboards:** Wired to live model rankings to automatically route tasks to the best-performing AI for a specific job.
- [ ] **Fusion (Skill Corpus v1):** Ingesting decision frameworks, personas, and logic trees to give the agent expert-level reasoning.
- [ ] **Fission (Plugin Corpus v1):** Building a modular plugin system with one-command updates.
- [ ] **Ingestor (Repo-Mining):** An automated phase that clones, analyzes, and proposes native re-implementations of external open-source tools.
- [ ] **TUI (Terminal UI):** A rich terminal interface showing Key Farm telemetry, model-routing visualizations, and memory activity feeds.
- [ ] **Automated Bug-Bounty Recon:** Autonomous agents that scan targets for vulnerabilities, with legal scope safety asserted strictly in code.
- [ ] **Autonomous Web Builder:** Agentic generation of high-end, anti-slop web interfaces directly from terminal prompts.
- [ ] **McOmokaro Synthesis:** The grand unification — reverting external research into native Rust, one organ at a time.
- [ ] **Web Front Door:** A free-hosted web interface accessible from any browser.
- [ ] **One-line curl installer:** `curl -fsSL mcharness.sh | bash` for instant deployment on any Linux machine.

## ⭐ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=McOmokaro/McHarness&type=Date)](https://star-history.com/#McOmokaro/McHarness&Date)

## License

This project is under a **Custom Proprietary Evaluation License**. See the `LICENSE` file for details.