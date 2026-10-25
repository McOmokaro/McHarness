# REA Integration

> "The best way to predict the future is to implement it." - Steve Wozniak

## Overview

The Reverse Engineer Anything (REA) MCP server is deeply integrated into the McHarness stack, providing over fifty native analysis tools that extend the agent's capabilities across native, managed, Electron/JavaScript, packaged, firmware, and browser targets. This integration follows the Open Standard Skills framework at agentskills.io, making skills portable, shareable, and community-contributable.

## Architecture

### MCP Server Integration Layer

The REA MCP server connects to McHarness through the standard Model Context Protocol (MCP), acting as a bridge between the agent's LLM and the native analysis tools. The integration consists of:

1. **Tool Registry**: Automatically discovers and registers all 50+ REA tools at startup, categorizing them by target type (native, managed, Electron, firmware, browser).

2. **Skill Interface**: Provides a unified `execute_rea_tool` function that the LLM can call, which maps to the appropriate REA backend based on the target type and desired analysis.

3. **Result Formatting**: All REA tool outputs are normalized to a consistent JSON schema, making it easy for the LLM to parse and reason about results.

### Available REA Tools (50+)

The integration provides tools for:

- **Native Binary Analysis**: Disassembly, symbol extraction, string analysis, and control flow graph generation
- **Managed Code Decompilation**: .NET and Java decompilers with control flow and data flow analysis
- **Electron/JavaScript Analysis**: Browser extension deobfuscation, Node.js module analysis, and Electron main/renderer process inspection
- **Firmware Analysis**: Flash image extraction, RTOS task analysis, and peripheral register mapping
- **Browser Page Extraction**: Vision, extraction, and interaction capabilities for web pages
- **Runtime Evidence Collection**: Symbol tables, decompiled code, pseudocode, and network capture analysis

## Technical Implementation

### Configuration

The REA integration is configured in `docusaurus.config.js` and initializes with the following settings:

```javascript
// REA MCP server integration
reaIntegration: {
  serverUrl: 'http://localhost:8080',
  autoRegisterTools: true,
  toolCategories: ['native', 'managed', 'electron', 'firmware', 'browser'],
  resultCacheTTL: 300, // seconds
}
```

### Tool Execution Flow

1. **LLM Request**: The LLM calls `execute_rea_tool({name: 'disassemble', binary: '/path/to/binary'})`

2. **Router**: The request is routed to the appropriate backend based on the `binary` path analysis:
   - Local file → Local rizin/gdb backend
   - Remote URL → Daytona/Modal backend
   - Docker container → Docker backend

3. **Execution**: The selected backend executes the analysis using the REA framework, which may involve:
   - Starting a rizin or gdb session
   - Loading the target binary
   - Running analysis plugins
   - Collecting evidence (symbols, decompiled code, pseudocode)

4. **Result Normalization**: Raw output is converted to the standard schema:
   ```json
   {
     "tool": "disassemble",
     "status": "success",
     "architecture": "x86_64",
     "functions": [...],
     "symbols": [...],
     "metadata": {...}
   }
   ```

5. **LLM Processing**: The normalized result is presented to the LLM, which can then:
   - Make further tool calls
   - Generate reports or summaries
   - Plan next steps in the analysis workflow

### Cross-Session Memory

The REA integration leverages McHarness's persistent memory system to:

- Cache analysis results across sessions
- Build a growing model of frequently analyzed patterns
- Provide FTS5 cross-session recall with LLM summarization
- Nudge the user to persist important discoveries

### Honkie Dialectic User Modeling

The integration incorporates Honcho dialectic user modeling to:

- Track which analysis tools the user favors
- Suggest relevant tools based on previous work
- Adapt the tool recommendation algorithm over time
- Create a personalized skill library from the user's analysis patterns

## Getting Started with REA Tools

### Basic Usage

```javascript
// Disassemble a binary
const result = await execute_rea_tool({
  name: 'disassemble',
  binary: '/path/to/binary',
  architecture: 'x86_64'
});

// Decompile managed code
const result = await execute_rea_tool({
  name: 'decompile',
  language: 'java',
  source: '/path/to/source.jar'
});

// Analyze firmware
const result = await execute_rea_tool({
  name: 'firmware-analysis',
  image: '/path/to/firmware.img',
  rtos: 'freeRTOS'
});
```

### Common Patterns

1. **Exploratory Analysis**: Start with high-level tools and drill down
   ```javascript
   // First get an overview
   const overview = await execute_rea_tool({name: 'overview', binary: '/path/to/binary'});
   // Then drill into specific functions
   const funcs = await execute_rea_tool({name: 'functions', binary: '/path/to/binary'});
   ```

2. **Parallel Analysis**: Run multiple tools simultaneously
   ```javascript
   const [disasm, symbols, strings] = await Promise.all([
     execute_rea_tool({name: 'disassemble', binary: '/path/to/binary'}),
     execute_rea_tool({name: 'extract-symbols', binary: '/path/to/binary'}),
     execute_rea_tool({name: 'extract-strings', binary: '/path/to/binary'})
   ]);
   ```

3. **Incremental Refinement**: Use results from one tool to focus another
   ```javascript
   // Use disassembly to focus string extraction
   const funcs = (await execute_rea_tool({name: 'disassemble', binary: '/path/to/binary'})).functions;
   const strings = await execute_rea_tool({
     name: 'extract-strings',
     binary: '/path/to/binary',
     functions: funcs.slice(0, 10) // Focus on first 10 functions
   });
   ```

## Integration with McHarness Features

### Bot Mode Integration

REA tools can be called from Bot Mode, allowing specialist bots to perform analysis:

```
@McHarness analyze this binary: /path/to/app
```

### Skill Creation

Users can create reusable skills from REA analysis patterns:

```javascript
// Create a skill from a repeated analysis pattern
create_skill({
  name: 'find-vulnerabilities',
  description: 'Find potential security vulnerabilities in binary',
  toolchain: ['disassemble', 'extract-strings', 'identify-sink-functions'],
  pattern: 'repeated-analysis'
})
```

### Memory Persistence

Analysis results are automatically persisted to memory, enabling:
- Cross-session reuse of previous analyses
- Summary generation of complex analyses
- Pattern recognition across multiple binaries
- Skill recommendations based on analysis history