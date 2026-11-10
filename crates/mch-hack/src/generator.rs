//! Code + pitch-deck generation (agent-facing stubs).

use crate::triage::Project;
use serde::{Deserialize, Serialize};

/// A specification for generated code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSpec {
    pub project_name: String,
    pub language: String,
    pub features: Vec<String>,
    pub acceptance_criteria: Vec<String>,
}

/// The generated code bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBundle {
    pub files: Vec<GeneratedFile>,
    pub entrypoint: String,
}

/// A single generated file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
}

/// Generates a skeleton code bundle from a spec.
pub struct CodeGenerator {
    /// If false, generation is a deterministic scaffold (no LLM).
    pub use_llm: bool,
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self { use_llm: false }
    }

    pub async fn generate(&self, spec: &CodeSpec) -> anyhow::Result<CodeBundle> {
        let mut files = Vec::new();
        let readme = format!(
            "# {}\n\nLanguage: {}\n\n## Features\n{}\n\n## Acceptance Criteria\n{}\n",
            spec.project_name,
            spec.language,
            spec.features
                .iter()
                .map(|f| format!("- {f}"))
                .collect::<Vec<_>>()
                .join("\n"),
            spec.acceptance_criteria
                .iter()
                .map(|c| format!("- [ ] {c}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        files.push(GeneratedFile {
            path: "README.md".into(),
            contents: readme,
        });
        files.push(GeneratedFile {
            path: format!("src/main.{}", ext_for(&spec.language)),
            contents: "// generated skeleton\nfn main() {}\n".into(),
        });
        Ok(CodeBundle {
            files,
            entrypoint: "src/main".into(),
        })
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

fn ext_for(language: &str) -> &'static str {
    match language.to_ascii_lowercase().as_str() {
        "rust" => "rs",
        "python" => "py",
        "go" => "go",
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        _ => "txt",
    }
}

/// A 3-minute pitch deck.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitchDeck {
    pub title: String,
    pub slides: Vec<PitchSlide>,
    /// Target spoken duration.
    pub duration_seconds: u32,
}

/// One slide of the pitch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PitchSlide {
    pub heading: String,
    pub bullets: Vec<String>,
}

/// Builds a rubric-aligned 3-minute pitch deck.
pub struct PitchDeckGenerator;

impl PitchDeckGenerator {
    pub fn new() -> Self {
        Self
    }

    pub async fn generate(&self, project: &Project) -> anyhow::Result<PitchDeck> {
        let slides = vec![
            PitchSlide {
                heading: "The Problem".into(),
                bullets: vec!["Who hurts, and how much".into()],
            },
            PitchSlide {
                heading: "Our Solution".into(),
                bullets: project.features.clone(),
            },
            PitchSlide {
                heading: "Tech".into(),
                bullets: project.tech_stack.clone(),
            },
            PitchSlide {
                heading: "Traction / Demo".into(),
                bullets: vec![if project.has_demo {
                    "Live demo".into()
                } else {
                    "Demo planned".into()
                }],
            },
            PitchSlide {
                heading: "Ask".into(),
                bullets: vec!["What we need next".into()],
            },
        ];
        Ok(PitchDeck {
            title: project.name.clone(),
            slides,
            duration_seconds: 180,
        })
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

    #[tokio::test]
    async fn generates_bundle_and_deck() {
        let gen = CodeGenerator::new();
        let spec = CodeSpec {
            project_name: "Demo".into(),
            language: "rust".into(),
            features: vec!["login".into()],
            acceptance_criteria: vec!["users can log in".into()],
        };
        let bundle = gen.generate(&spec).await.unwrap();
        assert!(bundle.files.iter().any(|f| f.path == "README.md"));

        let deck = PitchDeckGenerator::new()
            .generate(&Project::new("Demo"))
            .await
            .unwrap();
        assert_eq!(deck.duration_seconds, 180);
    }
}
