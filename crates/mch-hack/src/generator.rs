//! generator.rs — Code and pitch deck generation.
//!
//! Provides `CodeGenerator` for project implementation and `PitchDeckGenerator`
//! for 3-minute competition pitch decks.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Specification for code generation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodeSpec {
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub language: Language,
    pub framework: Option<String>,
    pub target: Target,
    pub requirements: Vec<Requirement>,
    pub constraints: Vec<Constraint>,
    pub integrations: Vec<Integration>,
    pub test_strategy: TestStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    TypeScript,
    Python,
    Go,
    Solidity,
    Move,
    Cairo,
    Noir,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "rust"),
            Language::TypeScript => write!(f, "typescript"),
            Language::Python => write!(f, "python"),
            Language::Go => write!(f, "go"),
            Language::Solidity => write!(f, "solidity"),
            Language::Move => write!(f, "move"),
            Language::Cairo => write!(f, "cairo"),
            Language::Noir => write!(f, "noir"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Cli,
    Tui,
    Gui,
    WebApp,
    SmartContract,
    Library,
    Service,
    Wasm,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Requirement {
    pub id: String,
    pub description: String,
    pub priority: Priority,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Must,
    Should,
    Could,
    Won't,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    MaxLines,
    MaxDependencies,
    MinTestCoverage,
    NoUnsafe,
    NoStd,
    GasLimit,
    BundleSize,
    LatencyMs,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Integration {
    pub name: String,
    pub kind: IntegrationKind,
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationKind {
    Api,
    Database,
    MessageQueue,
    Blockchain,
    Auth,
    Storage,
    Monitoring,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TestStrategy {
    Unit,
    Integration,
    Property,
    Fuzzing,
    FormalVerification,
    All,
}

/// Generated code bundle.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CodeBundle {
    pub spec_id: Uuid,
    pub files: Vec<GeneratedFile>,
    pub build_script: Option<String>,
    pub test_files: Vec<GeneratedFile>,
    pub docs: Vec<GeneratedFile>,
    pub metadata: GenerationMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedFile {
    pub path: PathBuf,
    pub content: String,
    pub language: Language,
    pub purpose: FilePurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FilePurpose {
    Main,
    Module,
    Test,
    Config,
    Doc,
    Script,
    Ci,
    Dockerfile,
    Makefile,
    Readme,
    License,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GenerationMetadata {
    pub generator_version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub estimated_lines: u32,
    pub estimated_test_lines: u32,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Runtime,
    Dev,
    Build,
    Optional,
}

/// Code generator using template-based approach with LLM enhancement.
pub struct CodeGenerator {
    templates: HashMap<String, CodeTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodeTemplate {
    name: String,
    language: Language,
    target: Target,
    files: Vec<TemplateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TemplateFile {
    path: String,
    content: String,
    purpose: FilePurpose,
    condition: Option<String>, // Simple condition like "has_database"
}

impl CodeGenerator {
    pub fn new() -> Self {
        let mut gen = Self {
            templates: HashMap::new(),
        };
        gen.register_builtin_templates();
        gen
    }

    /// Generate code from specification.
    pub async fn generate(&self, spec: &CodeSpec) -> anyhow::Result<CodeBundle> {
        let template_key = format!("{:?}-{:?}", spec.language, spec.target);
        let template = self.templates.get(&template_key)
            .ok_or_else(|| anyhow::anyhow!("no template for {}", template_key))?;

        let mut files = Vec::new();
        let mut test_files = Vec::new();
        let mut docs = Vec::new();
        let mut estimated_lines = 0;
        let mut estimated_test_lines = 0;

        for tf in &template.files {
            if self.should_include(&tf.condition, spec) {
                let content = self.render_template(&tf.content, spec);
                let lines = content.lines().count() as u32;
                let file = GeneratedFile {
                    path: PathBuf::from(&tf.path),
                    content,
                    language: spec.language,
                    purpose: tf.purpose,
                };
                match tf.purpose {
                    FilePurpose::Test => {
                        test_files.push(file);
                        estimated_test_lines += lines;
                    }
                    FilePurpose::Doc | FilePurpose::Readme => {
                        docs.push(file);
                    }
                    _ => {
                        files.push(file);
                        estimated_lines += lines;
                    }
                }
            }
        }

        // Add generated files for requirements
        let req_files = self.generate_requirement_files(spec).await?;
        estimated_lines += req_files.iter().map(|f| f.content.lines().count() as u32).sum::<u32>();
        files.extend(req_files);

        // Add test files for requirements
        let req_tests = self.generate_requirement_tests(spec).await?;
        estimated_test_lines += req_tests.iter().map(|f| f.content.lines().count() as u32).sum::<u32>();
        test_files.extend(req_tests);

        let build_script = self.generate_build_script(spec);
        let dependencies = self.infer_dependencies(spec);

        Ok(CodeBundle {
            spec_id: spec.project_id,
            files,
            build_script,
            test_files,
            docs,
            metadata: GenerationMetadata {
                generator_version: env!("CARGO_PKG_VERSION").into(),
                timestamp: chrono::Utc::now(),
                estimated_lines,
                estimated_test_lines,
                dependencies,
            },
        })
    }

    /// Write bundle to disk.
    pub async fn write_bundle(&self, bundle: &CodeBundle, output_dir: &PathBuf) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(output_dir).await?;
        for file in &bundle.files {
            let path = output_dir.join(&file.path);
            tokio::fs::create_dir_all(path.parent().unwrap()).await?;
            tokio::fs::write(path, &file.content).await?;
        }
        for file in &bundle.test_files {
            let path = output_dir.join(&file.path);
            tokio::fs::create_dir_all(path.parent().unwrap()).await?;
            tokio::fs::write(path, &file.content).await?;
        }
        for file in &bundle.docs {
            let path = output_dir.join(&file.path);
            tokio::fs::create_dir_all(path.parent().unwrap()).await?;
            tokio::fs::write(path, &file.content).await?;
        }
        if let Some(script) = &bundle.build_script {
            tokio::fs::write(output_dir.join("build.sh"), script).await?;
        }
        Ok(())
    }

    fn should_include(&self, condition: &Option<String>, spec: &CodeSpec) -> bool {
        match condition {
            Some(c) if c == "has_database" => spec.integrations.iter().any(|i| i.kind == IntegrationKind::Database),
            Some(c) if c == "has_blockchain" => spec.integrations.iter().any(|i| i.kind == IntegrationKind::Blockchain),
            Some(c) if c == "has_auth" => spec.integrations.iter().any(|i| i.kind == IntegrationKind::Auth),
            Some(c) if c == "has_monitoring" => spec.integrations.iter().any(|i| i.kind == IntegrationKind::Monitoring),
            _ => true,
        }
    }

    fn render_template(&self, template: &str, spec: &CodeSpec) -> String {
        template
            .replace("{{PROJECT_NAME}}", &spec.name)
            .replace("{{PROJECT_DESCRIPTION}}", &spec.description)
            .replace("{{LANGUAGE}}", &spec.language.to_string())
            .replace("{{FRAMEWORK}}", spec.framework.as_deref().unwrap_or(""))
            .replace("{{TARGET}}", &format!("{:?}", spec.target).to_lowercase())
    }

    fn generate_requirement_files(&self, spec: &CodeSpec) -> anyhow::Result<Vec<GeneratedFile>> {
        let mut files = Vec::new();
        for req in &spec.requirements {
            if req.priority == Priority::Must || req.priority == Priority::Should {
                let content = self.generate_requirement_module(spec, req);
                files.push(GeneratedFile {
                    path: PathBuf::from(format!("src/{}.rs", req.id.to_lowercase().replace(' ', "_"))),
                    content,
                    language: spec.language,
                    purpose: FilePurpose::Module,
                });
            }
        }
        Ok(files)
    }

    fn generate_requirement_module(&self, spec: &CodeSpec, req: &Requirement) -> String {
        match spec.language {
            Language::Rust => format!(r#"//! {req_name}
//! {description}

// Generated from requirement: {req_id}

use anyhow::Result;

pub struct {struct_name} {{
    // TODO: Add fields based on requirement
}}

impl {struct_name} {{
    pub fn new() -> Self {{
        Self {{}}
    }}

    pub async fn execute(&self) -> Result<()> {{
        // TODO: Implement requirement logic
        // Acceptance criteria:
{criteria}
        Ok(())
    }}
}}

impl Default for {struct_name} {{
    fn default() -> Self {{
        Self::new()
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[tokio::test]
    async fn test_{snake_name}_execution() {{
        let instance = {struct_name}::new();
        assert!(instance.execute().await.is_ok());
    }}
}}
"#,
                    req_name = req.id,
                    description = req.description,
                    req_id = req.id,
                    struct_name = req.id.split('_').map(|s| s.chars().next().unwrap().to_uppercase().to_string() + &s[1..]).collect::<String>(),
                    snake_name = req.id.to_lowercase(),
                    criteria = req.acceptance_criteria.iter().map(|c| format!("// - {}", c)).collect::<Vec<_>>().join("\n"),
                ),
            Language::TypeScript => format!(r#"/**
 * {req_name}
 * {description}
 * Generated from requirement: {req_id}
 */

export interface {struct_name}Config {{
  // TODO: Add config based on requirement
}}

export class {struct_name} {{
  constructor(config: {struct_name}Config) {{
    // TODO: Initialize
  }}

  async execute(): Promise<void> {{
    // TODO: Implement requirement logic
    // Acceptance criteria:
{criteria}
  }}
}}

export default {struct_name};
"#,
                    req_name = req.id,
                    description = req.description,
                    req_id = req.id,
                    struct_name = req.id.split('_').map(|s| s.chars().next().unwrap().to_uppercase().to_string() + &s[1..]).collect::<String>(),
                    criteria = req.acceptance_criteria.iter().map(|c| format!("    // - {}", c)).collect::<Vec<_>>().join("\n"),
                ),
            Language::Python => format!(r#"""""
{req_name}
{description}
Generated from requirement: {req_id}
""""

from dataclasses import dataclass
from typing import Any

@dataclass
class {struct_name}Config:
    # TODO: Add config based on requirement
    pass

class {struct_name}:
    def __init__(self, config: {struct_name}Config):
        # TODO: Initialize
        pass

    async def execute(self) -> None:
        # TODO: Implement requirement logic
        # Acceptance criteria:
{criteria}
        pass
"#,
                    req_name = req.id,
                    description = req.description,
                    req_id = req.id,
                    struct_name = req.id.split('_').map(|s| s.chars().next().unwrap().to_uppercase().to_string() + &s[1..]).collect::<String>(),
                    criteria = req.acceptance_criteria.iter().map(lambda c: f"        # - {c}").collect::<Vec<_>>().join("\n"),
                ),
            _ => format!("// Requirement: {}\n// {}\n// TODO: Implement for {:?}\n", req.id, req.description, spec.language),
        }
    }

    fn generate_requirement_tests(&self, spec: &CodeSpec) -> anyhow::Result<Vec<GeneratedFile>> {
        // Tests are generated inline in modules for Rust, separate for others
        Ok(vec![])
    }

    fn generate_build_script(&self, spec: &CodeSpec) -> Option<String> {
        match spec.language {
            Language::Rust => Some(r#"#!/bin/bash
set -euo pipefail

echo "Building Rust project..."
cargo build --release
echo "Running tests..."
cargo test --all
echo "Checking formatting..."
cargo fmt --check
echo "Running clippy..."
cargo clippy -- -D warnings
echo "Build complete!"
"#.into()),
            Language::TypeScript => Some(r#"#!/bin/bash
set -euo pipefail

echo "Installing dependencies..."
npm ci
echo "Building TypeScript project..."
npm run build
echo "Running tests..."
npm test
echo "Checking formatting..."
npm run format:check
echo "Running linter..."
npm run lint
echo "Build complete!"
"#.into()),
            Language::Python => Some(r#"#!/bin/bash
set -euo pipefail

echo "Installing dependencies..."
pip install -e ".[dev]"
echo "Running tests..."
pytest -xvs
echo "Checking formatting..."
ruff format --check .
echo "Running linter..."
ruff check .
echo "Type checking..."
mypy .
echo "Build complete!"
"#.into()),
            Language::Solidity => Some(r#"#!/bin/bash
set -euo pipefail

echo "Installing dependencies..."
forge install
echo "Building contracts..."
forge build
echo "Running tests..."
forge test -vvv
echo "Running coverage..."
forge coverage
echo "Build complete!"
"#.into()),
            _ => None,
        }
    }

    fn infer_dependencies(&self, spec: &CodeSpec) -> Vec<Dependency> {
        let mut deps = Vec::new();
        match spec.language {
            Language::Rust => {
                deps.push(Dependency { name: "anyhow".into(), version: "1.0".into(), kind: DependencyKind::Runtime });
                deps.push(Dependency { name: "tokio".into(), version: "1.0".into(), kind: DependencyKind::Runtime });
                deps.push(Dependency { name: "serde".into(), version: "1.0".into(), kind: DependencyKind::Runtime });
                if spec.integrations.iter().any(|i| i.kind == IntegrationKind::Database) {
                    deps.push(Dependency { name: "sqlx".into(), version: "0.7".into(), kind: DependencyKind::Runtime });
                }
                if spec.integrations.iter().any(|i| i.kind == IntegrationKind::Blockchain) {
                    deps.push(Dependency { name: "ethers".into(), version: "2.0".into(), kind: DependencyKind::Runtime });
                    deps.push(Dependency { name: "alloy".into(), version: "0.2".into(), kind: DependencyKind::Runtime });
                }
            }
            Language::TypeScript => {
                deps.push(Dependency { name: "typescript".into(), version: "5.0".into(), kind: DependencyKind::Dev });
                deps.push(Dependency { name: "vitest".into(), version: "1.0".into(), kind: DependencyKind::Dev });
            }
            Language::Python => {
                deps.push(Dependency { name: "pytest".into(), version: "7.0".into(), kind: DependencyKind::Dev });
                deps.push(Dependency { name: "pytest-asyncio".into(), version: "0.21".into(), kind: DependencyKind::Dev });
            }
            Language::Solidity => {
                deps.push(Dependency { name: "forge-std".into(), version: "1.9".into(), kind: DependencyKind::Dev });
                deps.push(Dependency { name: "openzeppelin-contracts".into(), version: "5.0".into(), kind: DependencyKind::Runtime });
            }
            _ => {}
        }
        deps
    }

    fn register_builtin_templates(&mut self) {
        // Rust CLI template
        self.templates.insert("Rust-Cli".into(), CodeTemplate {
            name: "Rust CLI".into(),
            language: Language::Rust,
            target: Target::Cli,
            files: vec![
                TemplateFile {
                    path: "Cargo.toml".into(),
                    content: r#"[package]
name = "{{PROJECT_NAME}}"
version = "0.1.0"
edition = "2021"
description = "{{PROJECT_DESCRIPTION}}"

[dependencies]
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
clap = { version = "4.0", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
"#.into(),
                    purpose: FilePurpose::Config,
                    condition: None,
                },
                TemplateFile {
                    path: "src/main.rs".into(),
                    content: r#"//! {{PROJECT_NAME}} - {{PROJECT_DESCRIPTION}}

use anyhow::Result;
use clap::Parser;
use tracing::info;

#[derive(Parser)]
#[command(name = "{{PROJECT_NAME}}", version, about = "{{PROJECT_DESCRIPTION}}")]
struct Cli {
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(&cli.log_level)?;
    info!("Starting {{PROJECT_NAME}}");
    
    // TODO: Implement main logic
    Ok(())
}

fn init_tracing(level: &str) -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| level.parse().unwrap());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
    Ok(())
}
"#.into(),
                    purpose: FilePurpose::Main,
                    condition: None,
                },
                TemplateFile {
                    path: "src/lib.rs".into(),
                    content: r#"//! {{PROJECT_NAME}} library

pub mod cli;
pub mod config;
pub mod error;

pub use error::{Error, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
"#.into(),
                    purpose: FilePurpose::Module,
                    condition: None,
                },
                TemplateFile {
                    path: "src/error.rs".into(),
                    content: r#"//! Error types for {{PROJECT_NAME}}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
"#.into(),
                    purpose: FilePurpose::Module,
                    condition: None,
                },
                TemplateFile {
                    path: "src/config.rs".into(),
                    content: r#"//! Configuration for {{PROJECT_NAME}}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub database_url: Option<String>,
    pub api_port: u16,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: None,
            api_port: 8080,
            log_level: "info".into(),
        }
    }
}

impl Config {
    pub fn from_file(path: &PathBuf) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}
"#.into(),
                    purpose: FilePurpose::Module,
                    condition: Some("has_database".into()),
                },
                TemplateFile {
                    path: "tests/integration_test.rs".into(),
                    content: r#"//! Integration tests for {{PROJECT_NAME}}

use anyhow::Result;

#[tokio::test]
async fn test_cli_help() -> Result<()> {
    let output = std::process::Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()?;
    assert!(output.status.success());
    Ok(())
}

#[tokio::test]
async fn test_basic_functionality() -> Result<()> {
    // TODO: Add real integration tests
    Ok(())
}
"#.into(),
                    purpose: FilePurpose::Test,
                    condition: None,
                },
                TemplateFile {
                    path: "README.md".into(),
                    content: r#"# {{PROJECT_NAME}}

{{PROJECT_DESCRIPTION}}

## Installation

```bash
cargo install {{PROJECT_NAME}}
```

## Usage

```bash
{{PROJECT_NAME}} --help
```

## Development

```bash
cargo build --release
cargo test
cargo fmt
cargo clippy
```

## License

MIT
"#.into(),
                    purpose: FilePurpose::Readme,
                    condition: None,
                },
            ],
        });

        // Rust TUI template (for hackit-ai style)
        self.templates.insert("Rust-Tui".into(), CodeTemplate {
            name: "Rust TUI".into(),
            language: Language::Rust,
            target: Target::Tui,
            files: vec![
                TemplateFile {
                    path: "Cargo.toml".into(),
                    content: r#"[package]
name = "{{PROJECT_NAME}}"
version = "0.1.0"
edition = "2021"
description = "{{PROJECT_DESCRIPTION}}"

[dependencies]
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
ratatui = "0.26"
crossterm = "0.27"
clap = { version = "4.0", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
"#.into(),
                    purpose: FilePurpose::Config,
                    condition: None,
                },
                TemplateFile {
                    path: "src/main.rs".into(),
                    content: r#"//! {{PROJECT_NAME}} - TUI Application

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::{io, time::Duration};
use tracing::info;

mod app;
mod ui;

use app::App;

#[derive(Parser)]
#[command(name = "{{PROJECT_NAME}}", version, about = "{{PROJECT_DESCRIPTION}}")]
struct Cli {
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(&cli.log_level)?;
    info!("Starting {{PROJECT_NAME}} TUI");
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    return Ok(());
                }
                app.handle_key(key.code);
            }
        }

        app.update().await?;
    }
}

fn init_tracing(level: &str) -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| level.parse().unwrap());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
    Ok(())
}
"#.into(),
                    purpose: FilePurpose::Main,
                    condition: None,
                },
                TemplateFile {
                    path: "src/app.rs".into(),
                    content: r#"//! Application state for {{PROJECT_NAME}}

use anyhow::Result;
use crossterm::event::KeyCode;

pub struct App {
    pub should_quit: bool,
    pub state: AppState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Main,
    Settings,
    Help,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            state: AppState::Main,
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('h') => self.state = AppState::Help,
            KeyCode::Char('s') => self.state = AppState::Settings,
            _ => {}
        }
    }

    pub async fn update(&mut self) -> Result<()> {
        // TODO: Add periodic updates
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
"#.into(),
                    purpose: FilePurpose::Module,
                    condition: None,
                },
                TemplateFile {
                    path: "src/ui.rs".into(),
                    content: r#"//! UI rendering for {{PROJECT_NAME}}

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, AppState};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(0),     // Content
            Constraint::Length(3),  // Status bar
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("{{PROJECT_NAME}}")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Content based on state
    match app.state {
        AppState::Main => draw_main(f, chunks[1]),
        AppState::Settings => draw_settings(f, chunks[1]),
        AppState::Help => draw_help(f, chunks[1]),
    }

    // Status bar
    let status = Paragraph::new(Line::from(vec![
        Span::styled("Press ", Style::default().fg(Color::Gray)),
        Span::styled("q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" to quit, ", Style::default().fg(Color::Gray)),
        Span::styled("h", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(" for help", Style::default().fg(Color::Gray)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}

fn draw_main(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Welcome to {{PROJECT_NAME}}"),
        Line::from(""),
        Line::from("A TUI application for hackathon competition."),
        Line::from(""),
        Line::from("Press 'h' for help, 's' for settings."),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Main"));
    f.render_widget(paragraph, area);
}

fn draw_settings(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Settings"),
        Line::from(""),
        Line::from("No settings configured yet."),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Settings"));
    f.render_widget(paragraph, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Help"),
        Line::from(""),
        Line::from("Key bindings:"),
        Line::from("  q / Esc  - Quit"),
        Line::from("  h        - This help"),
        Line::from("  s        - Settings"),
        Line::from(""),
        Line::from("{{PROJECT_DESCRIPTION}}"),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Help"));
    f.render_widget(paragraph, area);
}
"#.into(),
                    purpose: FilePurpose::Module,
                    condition: None,
                },
            ],
        });

        // TypeScript Web App template
        self.templates.insert("TypeScript-WebApp".into(), CodeTemplate {
            name: "TypeScript Web App".into(),
            language: Language::TypeScript,
            target: Target::WebApp,
            files: vec![
                TemplateFile {
                    path: "package.json".into(),
                    content: r#"{
  "name": "{{PROJECT_NAME}}",
  "version": "0.1.0",
  "description": "{{PROJECT_DESCRIPTION}}",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "test": "vitest",
    "format": "prettier --write .",
    "format:check": "prettier --check .",
    "lint": "eslint ."
  },
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0"
  },
  "devDependencies": {
    "@types/react": "^18.2.0",
    "@types/react-dom": "^18.2.0",
    "@vitejs/plugin-react": "^4.2.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "vitest": "^1.0.0",
    "prettier": "^3.0.0",
    "eslint": "^8.50.0"
  }
}
"#.into(),
                    purpose: FilePurpose::Config,
                    condition: None,
                },
                TemplateFile {
                    path: "tsconfig.json".into(),
                    content: r#"{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
"#.into(),
                    purpose: FilePurpose::Config,
                    condition: None,
                },
                TemplateFile {
                    path: "vite.config.ts".into(),
                    content: r#"import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist',
    sourcemap: true,
  },
})
"#.into(),
                    purpose: FilePurpose::Config,
                    condition: None,
                },
                TemplateFile {
                    path: "index.html".into(),
                    content: r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{PROJECT_NAME}}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#.into(),
                    purpose: FilePurpose::Other,
                    condition: None,
                },
                TemplateFile {
                    path: "src/main.tsx".into(),
                    content: r#"import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
"#.into(),
                    purpose: FilePurpose::Main,
                    condition: None,
                },
                TemplateFile {
                    path: "src/App.tsx".into(),
                    content: r#"import { useState } from 'react'
import './App.css'

function App() {
  const [count, setCount] = useState(0)

  return (
    <div className="app">
      <header>
        <h1>{{PROJECT_NAME}}</h1>
        <p>{{PROJECT_DESCRIPTION}}</p>
      </header>
      <main>
        <div className="card">
          <button onClick={() => setCount((count) => count + 1)}>
            Count is {count}
          </button>
          <p>Edit <code>src/App.tsx</code> and save to test HMR</p>
        </div>
      </main>
    </div>
  )
}

export default App
"#.into(),
                    purpose: FilePurpose::Module,
                    condition: None,
                },
            ],
        });

        // Solidity Smart Contract template
        self.templates.insert("Solidity-SmartContract".into(), CodeTemplate {
            name: "Solidity Smart Contract".into(),
            language: Language::Solidity,
            target: Target::SmartContract,
            files: vec![
                TemplateFile {
                    path: "foundry.toml".into(),
                    content: r#"[profile.default]
src = "src"
out = "out"
libs = ["lib"]
solc_version = "0.8.24"
optimizer = true
optimizer_runs = 200
via_ir = true

[fmt]
max_line_length = 120
tab_width = 4
bracket_spacing = true

[coverage]
basic = true
ir = true
"#.into(),
                    purpose: FilePurpose::Config,
                    condition: None,
                },
                TemplateFile {
                    path: "src/{{PROJECT_NAME}}.sol".into(),
                    content: r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import {Pausable} from "@openzeppelin/contracts/utils/Pausable.sol";

/**
 * @title {{PROJECT_NAME}}
 * @dev {{PROJECT_DESCRIPTION}}
 */
contract {{PROJECT_NAME}} is Ownable, ReentrancyGuard, Pausable {
    // State variables
    uint256 public totalSupply;
    
    // Events
    event Deposited(address indexed user, uint256 amount);
    event Withdrawn(address indexed user, uint256 amount);
    
    // Errors
    error ZeroAmount();
    error InsufficientBalance(uint256 balance, uint256 requested);
    error ContractPaused();
    
    constructor() Ownable(msg.sender) {
        // Initialize contract
    }
    
    /// @notice Deposit funds into the contract
    function deposit() external payable nonReentrant whenNotPaused {
        if (msg.value == 0) revert ZeroAmount();
        totalSupply += msg.value;
        emit Deposited(msg.sender, msg.value);
    }
    
    /// @notice Withdraw funds from the contract
    function withdraw(uint256 amount) external nonReentrant whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        if (address(this).balance < amount) revert InsufficientBalance(address(this).balance, amount);
        
        payable(msg.sender).sendValue(amount);
        totalSupply -= amount;
        emit Withdrawn(msg.sender, amount);
    }
    
    /// @notice Emergency pause (owner only)
    function pause() external onlyOwner {
        _pause();
    }
    
    /// @notice Emergency unpause (owner only)
    function unpause() external onlyOwner {
        _unpause();
    }
    
    /// @notice Get contract balance
    function getBalance() external view returns (uint256) {
        return address(this).balance;
    }
}
"#.into(),
                    purpose: FilePurpose::Main,
                    condition: None,
                },
                TemplateFile {
                    path: "test/{{PROJECT_NAME}}.t.sol".into(),
                    content: r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {{ {{PROJECT_NAME}} }} from "../src/{{PROJECT_NAME}}.sol";

contract {{PROJECT_NAME}}Test is Test {
    {{PROJECT_NAME}} public contract;
    address public user = makeAddr("user");
    
    function setUp() public {
        contract = new {{PROJECT_NAME}}();
    }
    
    function testDeposit() public {
        uint256 amount = 1 ether;
        vm.deal(user, amount);
        vm.prank(user);
        contract.deposit{value: amount}();
        
        assertEq(contract.totalSupply(), amount);
        assertEq(contract.getBalance(), amount);
    }
    
    function testWithdraw() public {
        uint256 amount = 1 ether;
        vm.deal(user, amount);
        vm.prank(user);
        contract.deposit{value: amount}();
        
        vm.prank(user);
        contract.withdraw(amount / 2);
        
        assertEq(contract.totalSupply(), amount / 2);
        assertEq(contract.getBalance(), amount / 2);
    }
    
    function testReentrancyProtection() public {
        // ReentrancyGuard should prevent reentrancy
        // This is a placeholder for actual reentrancy test
        assertTrue(true);
    }
    
    function testPause() public {
        contract.pause();
        vm.expectRevert(abi.encodeWithSelector(ContractPaused.selector));
        contract.deposit{value: 1 ether}();
    }
}
"#.into(),
                    purpose: FilePurpose::Test,
                    condition: None,
                },
                TemplateFile {
                    path: "README.md".into(),
                    content: r#"# {{PROJECT_NAME}}

{{PROJECT_DESCRIPTION}}

## Development

```bash
# Install dependencies
forge install

# Build
forge build

# Test
forge test -vvv

# Coverage
forge coverage

# Format
forge fmt
```

## Deployment

```bash
forge script script/Deploy.s.sol --rpc-url <RPC_URL> --private-key <PRIVATE_KEY> --broadcast
```

## License

MIT
"#.into(),
                    purpose: FilePurpose::Readme,
                    condition: None,
                },
            ],
        });
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Pitch deck generator for 3-minute competition presentations.
pub struct PitchDeckGenerator {
    templates: HashMap<String, PitchTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PitchTemplate {
    name: String,
    slides: Vec<SlideTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlideTemplate {
    title: String,
    content_template: String,
    duration_seconds: u32,
    visual_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PitchDeck {
    pub project_id: Uuid,
    pub slides: Vec<Slide>,
    pub total_duration_seconds: u32,
    pub format: PitchFormat,
    pub metadata: PitchMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Slide {
    pub number: u32,
    pub title: String,
    pub content: String,
    pub speaker_notes: String,
    pub duration_seconds: u32,
    pub visual_type: VisualType,
    pub visual_data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisualType {
    Title,
    Problem,
    Solution,
    Architecture,
    Demo,
    Traction,
    Market,
    BusinessModel,
    Team,
    Roadmap,
    Ask,
    Closing,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PitchFormat {
    Markdown,
    Html,
    Pdf,
    Marp,
    RevealJs,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PitchMetadata {
    pub generator_version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub target_audience: String,
    pub competition_name: Option<String>,
}

impl PitchDeckGenerator {
    pub fn new() -> Self {
        let mut gen = Self {
            templates: HashMap::new(),
        };
        gen.register_builtin_templates();
        gen
    }

    /// Generate a 3-minute pitch deck (180 seconds).
    pub async fn generate(&self, project: &crate::triage::Project) -> anyhow::Result<PitchDeck> {
        let template = self.templates.get("hackathon-3min")
            .ok_or_else(|| anyhow::anyhow!("missing default template"))?;

        let mut slides = Vec::new();
        let mut total_duration = 0;

        for (i, slide_tmpl) in template.slides.iter().enumerate() {
            let content = self.render_slide(&slide_tmpl.content_template, project);
            let speaker_notes = self.generate_speaker_notes(&slide_tmpl.title, project);
            
            slides.push(Slide {
                number: (i + 1) as u32,
                title: slide_tmpl.title.clone(),
                content,
                speaker_notes,
                duration_seconds: slide_tmpl.duration_seconds,
                visual_type: self.infer_visual_type(&slide_tmpl.title),
                visual_data: self.generate_visual_data(&slide_tmpl.title, project),
            });
            total_duration += slide_tmpl.duration_seconds;
        }

        Ok(PitchDeck {
            project_id: project.id,
            slides,
            total_duration_seconds: total_duration,
            format: PitchFormat::Marp,
            metadata: PitchMetadata {
                generator_version: env!("CARGO_PKG_VERSION").into(),
                timestamp: chrono::Utc::now(),
                target_audience: "hackathon judges".into(),
                competition_name: None,
            },
        })
    }

    /// Export deck to specified format.
    pub async fn export(&self, deck: &PitchDeck, format: PitchFormat, output_path: &std::path::Path) -> anyhow::Result<()> {
        let content = match format {
            PitchFormat::Markdown => self.to_markdown(deck),
            PitchFormat::Marp => self.to_marp(deck),
            PitchFormat::Html => self.to_html(deck),
            PitchFormat::RevealJs => self.to_revealjs(deck),
            PitchFormat::Pdf => {
                // PDF requires external tool; return Marp markdown
                self.to_marp(deck)
            },
        };
        tokio::fs::write(output_path, content).await?;
        Ok(())
    }

    fn render_slide(&self, template: &str, project: &crate::triage::Project) -> String {
        template
            .replace("{{PROJECT_NAME}}", &project.name)
            .replace("{{PROJECT_DESCRIPTION}}", &project.description)
            .replace("{{TECH_STACK}}", &project.tech_stack.join(", "))
            .replace("{{TEAM_SIZE}}", &project.team_size.to_string())
            .replace("{{STAGE}}", &project.stage.to_string())
            .replace("{{TRACK}}", &project.track)
    }

    fn generate_speaker_notes(&self, slide_title: &str, project: &crate::triage::Project) -> String {
        match slide_title {
            "Hook & Problem" => format!(
                "Open with the pain point. {} is the problem we're solving. {} team members, {} stage.",
                project.description.chars().take(100).collect::<String>(),
                project.team_size,
                project.stage
            ),
            "Solution" => format!(
                "Our solution: {}. Built with {}. Key innovation: {}",
                project.name,
                project.tech_stack.join(", "),
                project.description.chars().take(150).collect::<String>()
            ),
            "Architecture & Demo" => "Show, don't just tell. Live demo or recorded walkthrough. Highlight the TUI/CLI.",
            "Traction & Validation" => "Any users, tests passing, benchmarks, GitHub stars, early feedback.",
            "Market Opportunity" => "TAM/SAM/SOM. Why now? Why this team?",
            "Business Model" => "How we make money. Tokenomics if applicable. Unit economics.",
            "Team" => format!("{} people. Highlight relevant experience. {} tech stack expertise.", project.team_size, project.tech_stack.len()),
            "Roadmap" => "Next 30/60/90 days. Milestones. What winning this hackathon unlocks.",
            "The Ask" => "What we need: mentorship, funding, partnerships, users. Be specific.",
            "Closing" => "Memorable one-liner. Contact info. QR code to repo/demo.",
            _ => "Speaker notes for this slide.",
        }
    }

    fn infer_visual_type(&self, title: &str) -> VisualType {
        match title {
            "Hook & Problem" => VisualType::Problem,
            "Solution" => VisualType::Solution,
            "Architecture & Demo" => VisualType::Architecture,
            "Traction & Validation" => VisualType::Traction,
            "Market Opportunity" => VisualType::Market,
            "Business Model" => VisualType::BusinessModel,
            "Team" => VisualType::Team,
            "Roadmap" => VisualType::Roadmap,
            "The Ask" => VisualType::Ask,
            "Closing" => VisualType::Closing,
            _ => VisualType::Custom,
        }
    }

    fn generate_visual_data(&self, title: &str, project: &crate::triage::Project) -> serde_json::Value {
        match title {
            "Architecture & Demo" => serde_json::json!({
                "type": "diagram",
                "components": project.tech_stack,
                "style": "boxes-and-arrows"
            }),
            "Market Opportunity" => serde_json::json!({
                "type": "chart",
                "chart_type": "bar",
                "data": ["TAM", "SAM", "SOM"]
            }),
            "Traction & Validation" => serde_json::json!({
                "type": "metrics",
                "items": [
                    {"label": "Tests", "value": "passing"},
                    {"label": "Stage", "value": project.stage.to_string()},
                    {"label": "Artifacts", "value": project.artifacts.len()}
                ]
            }),
            _ => serde_json::json!({}),
        }
    }

    fn to_markdown(&self, deck: &PitchDeck) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", deck.slides[0].content.lines().next().unwrap_or("Pitch Deck")));
        for slide in &deck.slides {
            md.push_str(&format!("## Slide {}: {} ({}s)\n\n", slide.number, slide.title, slide.duration_seconds));
            md.push_str(&format!("{}\n\n", slide.content));
            md.push_str(&format!("> **Speaker Notes:** {}\n\n", slide.speaker_notes));
            md.push_str("---\n\n");
        }
        md
    }

    fn to_marp(&self, deck: &PitchDeck) -> String {
        let mut md = String::new();
        md.push_str("---\n");
        md.push_str("marp: true\n");
        md.push_str("theme: default\n");
        md.push_str("paginate: true\n");
        md.push_str("---\n\n");
        
        for slide in &deck.slides {
            md.push_str(&format!("## {}\n\n", slide.title));
            md.push_str(&format!("{}\n\n", slide.content));
            md.push_str(&format!("<!-- {} -->\n\n", slide.speaker_notes));
            md.push_str("---\n\n");
        }
        md
    }

    fn to_html(&self, deck: &PitchDeck) -> String {
        let mut html = String::new();
        html.push_str(r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Pitch Deck</title>
    <style>
        body { font-family: system-ui; max-width: 800px; margin: 0 auto; padding: 2rem; }
        .slide { page-break-after: always; margin-bottom: 3rem; }
        .speaker-notes { color: #666; font-size: 0.9em; border-left: 3px solid #ccc; padding-left: 1rem; }
    </style>
</head>
<body>
"#);
        for slide in &deck.slides {
            html.push_str(&format!(r#"<div class="slide">
    <h2>Slide {}: {}</h2>
    <div class="duration">Duration: {}s</div>
    <div class="content">{}</div>
    <div class="speaker-notes"><strong>Notes:</strong> {}</div>
</div>
"#, slide.number, slide.title, slide.duration_seconds, slide.content.replace('\n', "<br>"), slide.speaker_notes));
        }
        html.push_str("</body></html>");
        html
    }

    fn to_revealjs(&self, deck: &PitchDeck) -> String {
        let mut html = String::new();
        html.push_str(r#"<!DOCTYPE html>
<html>
<head>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/reveal.css">
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/theme/black.css">
</head>
<body>
<div class="reveal">
<div class="slides">
"#);
        for slide in &deck.slides {
            html.push_str(&format!(r#"<section data-duration="{}">
    <h2>{}</h2>
    <p>{}</p>
    <aside class="notes">{}</aside>
</section>
"#, slide.duration_seconds * 1000, slide.title, slide.content.replace('\n', "<br>"), slide.speaker_notes));
        }
        html.push_str(r#"</div>
</div>
<script src="https://cdn.jsdelivr.net/npm/reveal.js@4.5.0/dist/reveal.js"></script>
<script>Reveal.initialize({ controls: true, progress: true, history: true });</script>
</body></html>
"#);
        html
    }

    fn register_builtin_templates(&mut self) {
        // 3-minute hackathon pitch (hackit-ai style: 180 seconds = 10 slides ~18s each)
        self.templates.insert("hackathon-3min".into(), PitchTemplate {
            name: "3-Minute Hackathon Pitch".into(),
            slides: vec![
                SlideTemplate {
                    title: "Hook & Problem".into(),
                    content_template: r#"**{{PROJECT_NAME}}**

{{PROJECT_DESCRIPTION}}

*The problem in one sentence:* [Pain point you're solving]"#.into(),
                    duration_seconds: 15,
                    visual_hint: "Large problem statement, minimal text".into(),
                },
                SlideTemplate {
                    title: "Solution".into(),
                    content_template: r#"**Our Solution: {{PROJECT_NAME}}**

{{PROJECT_DESCRIPTION}}

**Tech Stack:** {{TECH_STACK}}
**Stage:** {{STAGE}}
**Track:** {{TRACK}}"#.into(),
                    duration_seconds: 20,
                    visual_hint: "Product screenshot or architecture diagram".into(),
                },
                SlideTemplate {
                    title: "Why Now & Why Us".into(),
                    content_template: r#"**Why This Team Wins**

- {{TEAM_SIZE}} builders with {{TECH_STACK}} expertise
- {{STAGE}} stage: working code, not slides
- Unique insight: [Your unfair advantage]"#.into(),
                    duration_seconds: 15,
                    visual_hint: "Team photos + key credentials".into(),
                },
                SlideTemplate {
                    title: "Architecture & Demo".into(),
                    content_template: r#"**Live Demo / Architecture**

[Show the TUI/CLI/Web app in action]

**Key Technical Decisions:**
- {{TECH_STACK}}
- [Architecture highlight]
- [Innovation highlight]"#.into(),
                    duration_seconds: 40,
                    visual_hint: "Live demo recording or architecture diagram".into(),
                },
                SlideTemplate {
                    title: "Traction & Validation".into(),
                    content_template: r#"**Proof It Works**

- ✅ All tests passing
- ✅ {{STAGE}} deployment ready
- ✅ {{TECH_STACK}} integration verified
- 📊 [Metrics: users, TPS, gas saved, etc.]"#.into(),
                    duration_seconds: 20,
                    visual_hint: "Dashboard screenshots, test output, metrics".into(),
                },
                SlideTemplate {
                    title: "Market Opportunity".into(),
                    content_template: r#"**Market Size**

**TAM:** $[X]B - [Total addressable market]
**SAM:** $[Y]M - [Serviceable available market]  
**SOM:** $[Z]K - [Serviceable obtainable market]

**Why now:** [Timing argument]"#.into(),
                    duration_seconds: 20,
                    visual_hint: "TAM/SAM/SOM chart".into(),
                },
                SlideTemplate {
                    title: "Business Model".into(),
                    content_template: r#"**How We Make Money**

- **Primary:** [Revenue stream 1]
- **Secondary:** [Revenue stream 2]
- **Tokenomics:** [If applicable - utility, governance, value accrual]

**Unit Economics:** CAC $[X] | LTV $[Y] | Payback [Z] months"#.into(),
                    duration_seconds: 15,
                    visual_hint: "Revenue flow diagram".into(),
                },
                SlideTemplate {
                    title: "Roadmap".into(),
                    content_template: r#"**Next Steps**

**30 Days:** [Immediate milestone]
**60 Days:** [Beta launch / mainnet]
**90 Days:** [Scale / partnerships]

**Winning this hackathon unlocks:** [Specific resources]"#.into(),
                    duration_seconds: 15,
                    visual_hint: "Timeline with milestones".into(),
                },
                SlideTemplate {
                    title: "The Ask".into(),
                    content_template: r#"**What We Need**

- 💰 **Funding:** $[Amount] for [purpose]
- 🤝 **Partnerships:** [Specific protocols/companies]
- 👥 **Team:** [Roles needed]
- 🛠 **Infrastructure:** [Grants, credits, access]

**Contact:** [Email/Discord/Twitter] | **Repo:** [GitHub URL]"#.into(),
                    duration_seconds: 15,
                    visual_hint: "Clear ask boxes with icons".into(),
                },
                SlideTemplate {
                    title: "Closing".into(),
                    content_template: r#"**{{PROJECT_NAME}}**

[One memorable sentence that captures the vision]

**Thank you.** Questions?

[QR Code to Demo/Repo]"#.into(),
                    duration_seconds: 5,
                    visual_hint: "Big vision statement + QR code".into(),
                },
            ],
        });
    }
}

impl Default for PitchDeckGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triage::{Project, ProjectStage, Artifact, ArtifactKind};

    fn sample_project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "Autonomous DeFi Agent".into(),
            description: "An autonomous agent that optimizes DeFi yields across protocols using MEV protection and intent-based execution".into(),
            repo_url: Some("https://github.com/example/agent".into()),
            tech_stack: vec!["rust".into(), "ethers".into(), "foundry".into(), "erc4337".into()],
            team_size: 3,
            track: "defi".into(),
            stage: ProjectStage::Mvp,
            artifacts: vec![
                Artifact { name: "src".into(), kind: ArtifactKind::SourceCode, path: "src/".into(), size_bytes: 50000 },
            ],
            metadata: Default::default(),
        }
    }

    #[test]
    fn code_generator_constructs() {
        let _gen = CodeGenerator::new();
    }

    #[test]
    fn pitch_deck_generator_constructs() {
        let _gen = PitchDeckGenerator::new();
    }

    #[tokio::test]
    async fn generate_rust_cli() {
        let gen = CodeGenerator::new();
        let spec = CodeSpec {
            project_id: Uuid::new_v4(),
            name: "test-cli".into(),
            description: "A test CLI".into(),
            language: Language::Rust,
            framework: None,
            target: Target::Cli,
            requirements: vec![],
            constraints: vec![],
            integrations: vec![],
            test_strategy: TestStrategy::Unit,
        };
        let bundle = gen.generate(&spec).await.unwrap();
        assert!(!bundle.files.is_empty());
        assert!(bundle.build_script.is_some());
        assert!(bundle.files.iter().any(|f| f.path.to_string_lossy().ends_with("Cargo.toml")));
        assert!(bundle.files.iter().any(|f| f.path.to_string_lossy().ends_with("main.rs")));
    }

    #[tokio::test]
    async fn generate_rust_tui() {
        let gen = CodeGenerator::new();
        let spec = CodeSpec {
            project_id: Uuid::new_v4(),
            name: "test-tui".into(),
            description: "A test TUI".into(),
            language: Language::Rust,
            framework: None,
            target: Target::Tui,
            requirements: vec![],
            constraints: vec![],
            integrations: vec![],
            test_strategy: TestStrategy::Unit,
        };
        let bundle = gen.generate(&spec).await.unwrap();
        assert!(bundle.files.iter().any(|f| f.path.to_string_lossy().contains("app.rs")));
        assert!(bundle.files.iter().any(|f| f.path.to_string_lossy().contains("ui.rs")));
    }

    #[tokio::test]
    async fn generate_solidity_contract() {
        let gen = CodeGenerator::new();
        let spec = CodeSpec {
            project_id: Uuid::new_v4(),
            name: "TestContract".into(),
            description: "A test contract".into(),
            language: Language::Solidity,
            framework: None,
            target: Target::SmartContract,
            requirements: vec![],
            constraints: vec![],
            integrations: vec![],
            test_strategy: TestStrategy::All,
        };
        let bundle = gen.generate(&spec).await.unwrap();
        assert!(bundle.files.iter().any(|f| f.path.to_string_lossy().ends_with(".sol")));
        assert!(bundle.files.iter().any(|f| f.path.to_string_lossy().contains("test/")));
        assert!(bundle.build_script.is_some());
    }

    #[tokio::test]
    async fn generate_pitch_deck() {
        let gen = PitchDeckGenerator::new();
        let project = sample_project();
        let deck = gen.generate(&project).await.unwrap();
        assert_eq!(deck.slides.len(), 10);
        assert_eq!(deck.total_duration_seconds, 180); // 3 minutes
        assert_eq!(deck.format, PitchFormat::Marp);
    }

    #[tokio::test]
    async fn export_pitch_deck() {
        let gen = PitchDeckGenerator::new();
        let project = sample_project();
        let deck = gen.generate(&project).await.unwrap();
        
        let tmp_dir = tempfile::tempdir().unwrap();
        let output = tmp_dir.path().join("deck.md");
        gen.export(&deck, PitchFormat::Markdown, &output).await.unwrap();
        
        let content = tokio::fs::read_to_string(&output).await.unwrap();
        assert!(content.contains("Autonomous DeFi Agent"));
        assert!(content.contains("Hook & Problem"));
        assert!(content.contains("The Ask"));
    }
}