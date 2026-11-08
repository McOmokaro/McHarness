# mch-browser: Native Computer-Use Agent

**Sources:**
- **OpenGCUA** (Tongyi-MAI/OpenGCUA) — Open Generalist CUA stack
- **OpAgent** (codefuse-ai/OpAgent) — Web navigation SOTA (71.6% WebArena)
- **EvoCUA** (meituan/EvoCUA) — Trained VLM, #1 open-source OSWorld (56.7%)
- **OmniPilot** (tejas-soni/OmniPilot) — IDE plugin with granular permissions

**Forge Target:** `crates/mch-browser`
**Status:** Forged

---

## Overview

mch-browser is a native Rust computer-use agent that implements the **OpenGCUA architecture**: code execution as action, accessibility tree as primary observation, and harness-to-model decision transfer. It combines the planner-grounder-reflector-summarizer pipeline from OpAgent with the cross-device SDK from OpenGCUA and the granular permission system from OmniPilot.

**Key insight:** Unlike GUI agents that click one element at a time, mch-browser lets the model write **Python programs** combining accessibility APIs, shell commands, and GUI actions with loops, branches, and conditionals. The model decides **what to observe, how to act, and how to organize work**.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        mch-browser                              │
├──────────────┬──────────────┬──────────────┬──────────────────┤
│  Observation │  Decision      │  Action        │  Control       │
│  (Eyes)      │  (Brain)       │  (Hands)       │  (Safety)      │
├──────────────┼──────────────┼──────────────┼──────────────────┤
│ Accessibility│ Planner        │ ActionExecutor │ PermissionMgr  │
│ Tree (CDP)   │ - Intent       │ - Click/Type   │ - Diff Viewer  │
│ Screenshot   │   + ActionType │ - Scroll       │ - Approve/Deny │
│ (VLM fallback)│ Grounder      │ - Navigate     │ - Allow Always │
│              │ - Coordinates  │ - Execute Code │                │
│              │ Reflector      │ - Screenshot   │ TodoManager    │
│              │ - Verify       │                │ - Hierarchical │
│              │ Summarizer     │ SkillManager   │ - Priorities   │
│              │ - Final answer │ - SKILL.md     │ - Subtasks     │
└──────────────┴──────────────┴──────────────┴──────────────────┘
```

### Core Components

| Module | Source | Purpose |
|--------|--------|---------|
| `cdp` | chromiumoxide | Chrome DevTools Protocol client |
| `observation` | OpenGCUA/OpAgent | Accessibility tree + VLM fallback |
| `planner` | OpAgent | High-level instruction + intent |
| `grounder` | OpAgent | Precise coordinates/parameters |
| `reflector` | OpAgent | Result verification + feedback |
| `summarizer` | OpAgent | Final answer generation |
| `action` | OpenGCUA/OpAgent | Code execution + GUI actions |
| `permissions` | OmniPilot | Granular human-in-the-loop |
| `skills` | OpenGCUA | SKILL.md workflow system |
| `todo` | OpenGCUA | Hierarchical task tracking |
| `sdk` | OpenGCUA | Cross-device unified interface |

---

## Observation Engine (The Eyes)

### Dual-Mode Observation

| Mode | Trigger | Latency | Use Case |
|------|---------|---------|----------|
| **Accessibility Tree** | Default (HTML apps) | ~50ms | Buttons, forms, tables, text |
| **Screenshot + VLM** | Canvas/WebGL/unknown | ~500ms | Games, maps, custom UI |
| **Hybrid** | Complex pages | ~550ms | Best of both |

```rust
let observation = sdk.observe().await?;

// Accessibility tree (deterministic, structured)
if let Some(tree) = observation.accessibility_tree {
    for elem in observation.interactive_elements {
        println!("[{}] {} @ {:?}", elem.role, elem.name, elem.bounds);
    }
}

// VLM description (semantic understanding)
if let Some(vlm) = observation.vlm_description {
    println!("Page summary: {}", vlm);
}
```

### Accessibility Tree Extraction

Uses CDP's `Accessibility.getFullAXTree` → structured `AxNode` tree with:
- **Role**: button, link, textbox, checkbox, etc.
- **Name/Description**: ARIA labels, text content
- **Bounds**: x, y, width, height (for coordinate grounding)
- **State**: focused, selected, disabled, editable
- **Attributes**: All HTML attributes

---

## Decision Pipeline (The Brain)

### 5-Stage Agent Loop (from OpAgent)

```
┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
│  Observe    │──▶│   Plan      │──▶│  Ground     │──▶│  Execute    │──▶│  Reflect    │
│  (CDP)      │   │  (Planner)  │   │ (Grounder)  │   │ (Executor)  │   │ (Reflector) │
└─────────────┘   └─────────────┘   └─────────────┘   └─────────────┘   └─────────────┘
       │                │                │                │                │
       ▼                ▼                ▼                ▼                ▼
  AxTree/Screenshot  Intent+Action   Coords/Selector  Click/Type/    Success/Fail
  + VLM description  Type+Params     + Code           Code/Navigate  + Feedback
                                                                    │
                                                                    ▼
                                                           ┌─────────────┐
                                                           │  Summarize  │
                                                           │ (Summarizer)│
                                                           └─────────────┘
```

### Planner (High-Level Decisions)

Generates `Plan` with:
- **Instruction**: Natural language action description
- **ActionType**: Click, Type, Scroll, Navigate, Wait, ExecuteCode
- **TargetElement**: Element ID from accessibility tree
- **Parameters**: Text, URL, scroll direction, code, etc.
- **Intent**: What this action accomplishes
- **DomainTips**: Injected expert strategies (shopping, admin, etc.)

```rust
let plan = sdk.plan("Search for 'rust tutorial' and click first result", None).await?;
// Plan { instruction: "Click search button", action_type: Click, target: "search-btn", ... }
```

### Grounder (Precise Parameters)

Converts high-level plan → executable parameters:
- **Click** → (x, y) coordinates or CSS selector
- **Type** → Selector + text content
- **Scroll** → Direction + pixel amount
- **Navigate** → URL
- **ExecuteCode** → Python program with loops/branches

Uses VLM (Qwen2.5-VL-72B post-trained) for coordinate grounding from screenshot.

### Reflector (Verification)

Post-execution analysis:
- **TaskDone**: Is the overall task complete?
- **Success**: Did action achieve intended effect?
- **Feedback**: Signal to planner for next step
- **Notes**: Key information for final answer
- **Correction**: Suggested fix if failed

### Summarizer (Final Answer)

Generates structured response from:
- Execution history (steps + reflections)
- Collected notes
- Final page state

---

## Action Executor (The Hands)

### Code Execution as Action (OpenGCUA Innovation)

Instead of one-at-a-time GUI operations, the model writes **Python programs**:

```python
# Model-generated code with loops, branches, checks
for item in page.find_all(".product-item"):
    if "target" in item.text.lower():
        item.click()
        break

# Combined with shell commands
import subprocess
result = subprocess.run(["git", "status"], capture_output=True)
if "clean" not in result.stdout:
    page.type("#commit-msg", "Auto-commit")
    page.click("#commit-btn")
```

**Advantages:**
- Intermediate results decide next operation without another model call
- Loops, conditionals, error handling in one execution
- Leverages model's existing programming skill

### Primitive Actions

| Action | Implementation |
|--------|----------------|
| Click | CDP `Mouse.click(x, y)` or `Page.click(selector)` |
| Type | `Page.type_str(text)` after focus |
| Scroll | `Mouse.wheel(dx, dy)` |
| Navigate | `Page.goto(url)` + wait for navigation |
| Wait | `tokio::time::sleep()` |
| ExecuteCode | `tokio::process::Command` (Python) |
| Screenshot | CDP `Page.captureScreenshot()` |

---

## Permission System (The Safety) — from OmniPilot

### Granular Human-in-the-Loop

| Operation | Approval Required | UI |
|-----------|-------------------|-----|
| File write | Yes | Diff Viewer (before/after) |
| Terminal command | Yes | Command preview + args |
| Network request | Yes | URL + method + headers |
| Browser navigation | Configurable | URL + current page |
| Read-only | No (auto-approve) | — |

### Decision Types

| Decision | Behavior |
|----------|----------|
| **Allow** | Allow this once |
| **Deny** | Block this once |
| **AllowOnce** | Allow this session |
| **AllowAlways** | Remember for this pattern |

```rust
let response = sdk.request_permission(PermissionRequest::FileWrite {
    path: "config.yaml".to_string(),
    content_preview: "debug: true\n".to_string(),
    diff: Some("- debug: false\n+ debug: true\n".to_string()),
}).await?;
```

---

## Skill System (The Workflows) — from OpenGCUA

### SKILL.md Format

```markdown
---
name: pdf-form
description: Fill PDF forms automatically
version: "1.0"
triggers: ["fill form", "pdf form"]
---

# PDF Form Filling Skill

## Instructions
1. Open PDF in Preview
2. Use accessibility tree to find form fields
3. Type values from provided data
4. Save completed form
```

### Skill Management

```rust
// Auto-loaded from skills_dirs
sdk.load_skills().await?;

// Get catalog for model context
let catalog = sdk.skill_catalog().await;
// [("pdf-form", "Fill PDF forms automatically"), ...]

// Load specific skill
if let Some(skill) = sdk.load_skill("pdf-form").await {
    // Inject into model context
}
```

---

## Todo Manager (The Organization) — from OpenGCUA

### Hierarchical Task Tracking

```rust
// Root task
let root = sdk.create_todo("Research travel options", None, TodoPriority::High).await?;

// Subtasks
sdk.create_subtask(root.id, "Compare flight prices", None, TodoPriority::Medium).await?;
sdk.create_subtask(root.id, "Check hotel availability", None, TodoPriority::Medium).await?;

// Update status
sdk.todo.set_status(subtask_id, TodoStatus::InProgress).await?;

// Get tree
let tree = sdk.todo.get_tree(root.id).await?;
```

### Model Integration

The model **states `<intent>` before each program** and maintains the todo list autonomously:

```rust
// Model-generated intent
<intent>Compare hotel prices across 3 sites</intent>
<code>
# Python code to check 3 hotel sites
for site in ["booking.com", "expedia.com", "hotels.com"]:
    page.goto(site)
    # ... extract prices
</code>
```

---

## Cross-Device SDK (The Portability) — from OpenGCUA

### Unified Interface

```python
from qwen_intelligence import Runtime

async def main():
    async with Runtime() as runtime:
        await runtime.add_macos(name="local-mac")
        async with runtime.session() as mu:
            mac = await mu.register("local-mac", name="mac")
            app = await mac.apps.get("com.apple.TextEdit")
            await mu.emit(await app.ui.ax.observe())  # Accessibility tree
```

### Adapters

| Platform | Adapter | Capabilities |
|----------|---------|--------------|
| macOS | CuaDriver.app | Full GUI + accessibility + shell |
| Linux | X11/Wayland + AT-SPI | Accessibility + shell |
| Windows | UI Automation | Accessibility + shell |
| Android | ADB + UIAutomator | Screenshot + tap + shell |
| Browser | CDP | Full DOM + accessibility + console |

### Rust Core + Python Bindings

- **Rust core**: `qwen-intelligence-sdk` (CDP, accessibility, action execution)
- **Python bindings**: `qwen_intelligence` (async API, device management)

---

## Integration Points

| McHarness Crate | Integration |
|-----------------|-------------|
| `mch-loop` | Drive browser agent loops (observe → plan → ground → execute → reflect) |
| `mch-brain` | Store browsing history, extracted facts, procedures as memory records |
| `mch-mcp` | Expose browser as MCP tools (`browser_navigate`, `browser_click`, `browser_screenshot`) |
| `mch-scope` | Gate dangerous operations (file writes, shell commands, navigation) |
| `mch-audit` | Log every browser action for reproducibility |
| `mch-rea` | Use REA tools for binary analysis during browser-based RE |

---

## Usage Example

```rust
use mch_browser::{BrowserSdk, SdkConfig, DeviceType, TodoPriority};

#[tokio::main]
async fn main() -> Result<()> {
    let config = SdkConfig {
        device_type: DeviceType::Browser,
        ..Default::default()
    };
    
    let mut sdk = BrowserSdk::new(config).await?;
    
    // Navigate
    sdk.navigate("https://example.com").await?;
    
    // Create task tracking
    let task = sdk.create_todo("Extract all links", None, TodoPriority::High).await?;
    
    // Autonomous loop
    let mut reflection = None;
    loop {
        let plan = sdk.plan("Find all links on page and save to file", reflection).await?;
        let grounding = sdk.ground(&plan).await?;
        let result = sdk.execute(&plan, &grounding).await?;
        
        let prev_obs = sdk.observe().await?;
        reflection = Some(sdk.reflect(&prev_obs, &grounding, "Extract all links").await?);
        
        if reflection.is_task_done { break; }
    }
    
    // Final summary
    let answer = sdk.summarize("Extract all links", &history, &notes).await?;
    println!("{}", answer);
    
    sdk.close().await?;
    Ok(())
}
```

---

## Files Reference

| File | Description |
|------|-------------|
| `crates/mch-browser/src/cdp.rs` | CDP client (chromiumoxide) |
| `crates/mch-browser/src/observation.rs` | Accessibility tree + VLM |
| `crates/mch-browser/src/planner.rs` | High-level planner |
| `crates/mch-browser/src/grounder.rs` | Coordinate/parameter grounding |
| `crates/mch-browser/src/reflector.rs` | Result verification |
| `crates/mch-browser/src/summarizer.rs` | Final answer generation |
| `crates/mch-browser/src/action.rs` | Code execution + GUI actions |
| `crates/mch-browser/src/permissions.rs` | OmniPilot-style permissions |
| `crates/mch-browser/src/skills.rs` | SKILL.md workflow system |
| `crates/mch-browser/src/todo.rs` | Hierarchical task tracking |
| `crates/mch-browser/src/sdk.rs` | Cross-device SDK |
| `docs/ingestor/browser.md` | This document |

---

## Open Questions / Future Work

- [ ] **Real CDP integration**: Complete chromiumoxide integration (currently placeholder)
- [ ] **VLM integration**: Connect to qwen-vl-max / GPT-4V for screenshot grounding
- [ ] **Cross-device adapters**: macOS (CuaDriver), Linux (AT-SPI), Android (ADB)
- [ ] **Skill marketplace**: SKILL.md registry with versioning
- [ ] **RL training**: Integrate with OpAgent's multi-agent RL framework
- [ ] **Benchmark**: Run on OSWorld / WebArena / WindowsAgentArena
- [ ] **Safety hardening**: Prompt injection defense, sandboxed code execution