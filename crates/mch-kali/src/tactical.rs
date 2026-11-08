//! Probabilistic Tactical Engine.
//!
//! Given a target, the planner:
//!   1. fingerprints it (whatweb / nmap),
//!   2. derives a tech stack,
//!   3. scores candidate tools by expected yield, and
//!   4. emits an ordered `TacticalPlan` of tool+flag invocations.
//!
//! The scoring is deterministic and inspectable (a probabilistic prior that a
//! human or an LLM can override); an [`FingerprintAnalyzer`] hook lets an LLM
//! refine the stack when one is available.

use crate::lazy::LazyLoader;
use crate::tools::{KaliTool, ToolCategory, Whatweb};

use serde::{Deserialize, Serialize};

/// Recognized technologies, ordered by specificity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechStack {
    WordPress,
    Drupal,
    Joomla,
    Php,
    NodeJs,
    Python,
    Java,
    DotNet,
    Nginx,
    Apache,
    Iis,
    Cloudflare,
    GraphQl,
    Generic,
}

impl TechStack {
    /// Map a whatweb plugin/tech token to a known stack.
    pub fn from_token(token: &str) -> Option<Self> {
        let t = token.to_ascii_lowercase();
        let m = if t.contains("wordpress") {
            TechStack::WordPress
        } else if t.contains("drupal") {
            TechStack::Drupal
        } else if t.contains("joomla") {
            TechStack::Joomla
        } else if t.contains("php") {
            TechStack::Php
        } else if t.contains("node") || t.contains("express") {
            TechStack::NodeJs
        } else if t.contains("python") || t.contains("django") || t.contains("flask") {
            TechStack::Python
        } else if t.contains("java") || t.contains("spring") || t.contains("tomcat") {
            TechStack::Java
        } else if t.contains("asp.net") || t.contains(".net") || t.contains("iis") {
            TechStack::DotNet
        } else if t.contains("nginx") {
            TechStack::Nginx
        } else if t.contains("apache") {
            TechStack::Apache
        } else if t.contains("cloudflare") {
            TechStack::Cloudflare
        } else if t.contains("graphql") {
            TechStack::GraphQl
        } else {
            return None;
        };
        Some(m)
    }

    /// WordPress-specific default wordlist.
    pub fn default_wordlist(&self) -> &'static str {
        match self {
            TechStack::WordPress => "/usr/share/wordlists/wpscan/wp-plugins.txt",
            TechStack::Drupal => "/usr/share/wordlists/seclists/Discovery/Web-Content/drupal.txt",
            _ => "/usr/share/wordlists/dirb/common.txt",
        }
    }
}

/// Result of fingerprinting a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetFingerprint {
    pub target: String,
    pub stacks: Vec<TechStack>,
    pub server: Option<String>,
    pub open_ports: Vec<u16>,
    /// Raw plugin tokens observed by whatweb.
    pub plugins: Vec<String>,
    /// Free-form signals an LLM analyzer may add.
    pub signals: Vec<String>,
}

impl TargetFingerprint {
    pub fn has(&self, stack: TechStack) -> bool {
        self.stacks.contains(&stack)
    }
}

/// A single planned tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedAction {
    pub tool: KaliTool,
    pub args: Vec<String>,
    /// Human-readable rationale ("WordPress detected → pivot to wpscan").
    pub rationale: String,
    /// 0.0–1.0 expected-yield score (the "probabilistic" weight).
    pub score: f64,
    pub category: ToolCategory,
}

/// The full plan for a target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalPlan {
    pub target: String,
    pub fingerprint: TargetFingerprint,
    /// Ordered best-first actions.
    pub actions: Vec<PlannedAction>,
}

/// Pluggable analyzer that can refine a heuristically-derived fingerprint
/// (e.g. an LLM reading the raw whatweb output).
#[async_trait::async_trait]
pub trait FingerprintAnalyzer: Send + Sync {
    async fn refine(
        &self,
        target: &str,
        raw: &serde_json::Value,
        base: TargetFingerprint,
    ) -> TargetFingerprint;
}

/// Default deterministic analyzer — no external calls.
pub struct HeuristicAnalyzer;

#[async_trait::async_trait]
impl FingerprintAnalyzer for HeuristicAnalyzer {
    async fn refine(
        &self,
        _target: &str,
        _raw: &serde_json::Value,
        base: TargetFingerprint,
    ) -> TargetFingerprint {
        base
    }
}

/// Tactical planner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalPlannerConfig {
    /// Probe ports with nmap during fingerprinting.
    pub nmap_fingerprint: bool,
    /// Use whatweb during fingerprinting.
    pub whatweb_fingerprint: bool,
    /// Default content-discovery wordlist for generic targets.
    pub generic_wordlist: String,
}

impl Default for TacticalPlannerConfig {
    fn default() -> Self {
        Self {
            nmap_fingerprint: false,
            whatweb_fingerprint: true,
            generic_wordlist: "/usr/share/wordlists/dirb/common.txt".to_string(),
        }
    }
}

/// The probabilistic tactical planner.
pub struct TacticalPlanner<'a> {
    loader: &'a LazyLoader,
    config: TacticalPlannerConfig,
    analyzer: Box<dyn FingerprintAnalyzer>,
}

impl<'a> TacticalPlanner<'a> {
    pub fn new(loader: &'a LazyLoader) -> Self {
        Self {
            loader,
            config: TacticalPlannerConfig::default(),
            analyzer: Box::new(HeuristicAnalyzer),
        }
    }

    pub fn with_config(mut self, config: TacticalPlannerConfig) -> Self {
        self.config = config;
        self
    }

    /// Install a custom (e.g. LLM-backed) analyzer.
    pub fn with_analyzer(mut self, analyzer: Box<dyn FingerprintAnalyzer>) -> Self {
        self.analyzer = analyzer;
        self
    }

    /// Fingerprint a target, then derive a plan.
    pub async fn plan(&self, target: &str) -> anyhow::Result<TacticalPlan> {
        let fingerprint = self.fingerprint(target).await?;
        let actions = self.derive_actions(&fingerprint);
        Ok(TacticalPlan {
            target: target.to_string(),
            fingerprint,
            actions,
        })
    }

    /// Fingerprint via whatweb (and optionally nmap).
    pub async fn fingerprint(&self, target: &str) -> anyhow::Result<TargetFingerprint> {
        let mut stacks = Vec::new();
        let mut plugins = Vec::new();
        let mut server = None;
        let mut open_ports = Vec::new();
        let mut raw = serde_json::Value::Null;

        if self.config.whatweb_fingerprint {
            let out = Whatweb::run(self.loader, target).await?;
            raw = out.parsed.clone();
            if let Some(arr) = out.parsed.as_array() {
                for entry in arr {
                    if let Some(plugs) = entry.get("plugins").and_then(|p| p.as_object()) {
                        for k in plugs.keys() {
                            plugins.push(k.clone());
                            if let Some(s) = TechStack::from_token(k) {
                                if !stacks.contains(&s) {
                                    stacks.push(s);
                                }
                            }
                        }
                    }
                }
            } else if let Some(plugs) = out.parsed.get("plugins").and_then(|p| p.as_array()) {
                for p in plugs {
                    if let Some(s) = p.as_str() {
                        plugins.push(s.to_string());
                        if let Some(st) = TechStack::from_token(s) {
                            if !stacks.contains(&st) {
                                stacks.push(st);
                            }
                        }
                    }
                }
            }
            if plugins.iter().any(|p| p.to_lowercase().contains("nginx")) {
                server = Some("nginx".to_string());
            }
        }

        if self.config.nmap_fingerprint {
            let out = crate::tools::Nmap::run(self.loader, target).await?;
            if let Some(ports) = out.parsed.get("ports").and_then(|p| p.as_array()) {
                for p in ports {
                    if let Some(port) = p.get("port").and_then(|v| v.as_u64()) {
                        open_ports.push(port as u16);
                    }
                }
            }
        }

        if stacks.is_empty() {
            stacks.push(TechStack::Generic);
        }

        let base = TargetFingerprint {
            target: target.to_string(),
            stacks,
            server,
            open_ports,
            plugins,
            signals: Vec::new(),
        };

        Ok(self.analyzer.refine(target, &raw, base).await)
    }

    /// Score tools against the fingerprint and produce ordered actions.
    pub fn derive_actions(&self, fp: &TargetFingerprint) -> Vec<PlannedAction> {
        let mut out = Vec::new();

        // Fingerprint-confirmation tools first (low risk, high info).
        out.push(PlannedAction {
            tool: KaliTool::Whatweb,
            args: Whatweb::args(&fp.target),
            rationale: "confirm technology fingerprint".into(),
            score: 0.55,
            category: ToolCategory::Fingerprint,
        });

        // WordPress pivot.
        if fp.has(TechStack::WordPress) {
            out.push(PlannedAction {
                tool: KaliTool::Wpscan,
                args: vec!["--url".into(), fp.target.clone(), "--enumerate".into(), "vp,vt,u".into()],
                rationale: "WordPress detected → pivot to wpscan".into(),
                score: 0.95,
                category: ToolCategory::Fingerprint,
            });
            out.push(PlannedAction {
                tool: KaliTool::Ffuf,
                args: vec![
                    "-u".into(),
                    format!("{}/FUZZ", fp.target.trim_end_matches('/')),
                    "-w".into(),
                    TechStack::WordPress.default_wordlist().into(),
                    "-of".into(),
                    "json".into(),
                    "-o".into(),
                    "/dev/stdout".into(),
                ],
                rationale: "WordPress detected → ffuf with WordPress plugin wordlist".into(),
                score: 0.9,
                category: ToolCategory::Fuzzing,
            });
        }

        // Drupal/Joomla pivots.
        if fp.has(TechStack::Drupal) || fp.has(TechStack::Joomla) {
            out.push(PlannedAction {
                tool: KaliTool::Nuclei,
                args: vec![
                    "-u".into(),
                    fp.target.clone(),
                    "-tags".into(),
                    "cms".into(),
                    "-severity".into(),
                    "critical,high,medium".into(),
                    "-jsonl".into(),
                ],
                rationale: "CMS detected → nuclei CMS template set".into(),
                score: 0.85,
                category: ToolCategory::Exploitation,
            });
        }

        // GraphQL endpoint.
        if fp.has(TechStack::GraphQl) {
            out.push(PlannedAction {
                tool: KaliTool::Ffuf,
                args: vec![
                    "-u".into(),
                    fp.target.clone(),
                    "-X".into(),
                    "POST".into(),
                    "-d".into(),
                    "{\"query\":\"{__typename}\"}".into(),
                    "-w".into(),
                    "/usr/share/seclists/Discovery/Web-Content/graphql.txt".into(),
                ],
                rationale: "GraphQL detected → introspection fuzz".into(),
                score: 0.8,
                category: ToolCategory::Fuzzing,
            });
        }

        // Dynamic-language backends → SQLi candidate.
        let dynamic = fp.has(TechStack::Php)
            || fp.has(TechStack::NodeJs)
            || fp.has(TechStack::Python)
            || fp.has(TechStack::Java)
            || fp.has(TechStack::DotNet);
        if dynamic {
            out.push(PlannedAction {
                tool: KaliTool::Sqlmap,
                args: vec![
                    "-u".into(),
                    fp.target.clone(),
                    "--batch".into(),
                    "--smart".into(),
                    "--level".into(),
                    "3".into(),
                    "--risk".into(),
                    "1".into(),
                ],
                rationale: "dynamic backend → SQLi probe via sqlmap".into(),
                score: 0.7,
                category: ToolCategory::Exploitation,
            });
        }

        // Generic content discovery (low risk baseline).
        out.push(PlannedAction {
            tool: KaliTool::Ffuf,
            args: vec![
                "-u".into(),
                format!("{}/FUZZ", fp.target.trim_end_matches('/')),
                "-w".into(),
                self.config.generic_wordlist.clone(),
                "-of".into(),
                "json".into(),
                "-o".into(),
                "/dev/stdout".into(),
                "-mc".into(),
                "200,301,302,401,403".into(),
            ],
            rationale: "baseline content discovery".into(),
            score: 0.5,
            category: ToolCategory::Fuzzing,
        });

        // High-severity vulnerability templates last (broad but noisy).
        out.push(PlannedAction {
            tool: KaliTool::Nuclei,
            args: Nuclei::args(&fp.target),
            rationale: "broad CVE/misconfig scan".into(),
            score: 0.6,
            category: ToolCategory::Exploitation,
        });

        // Order best-first.
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out
    }
}

use crate::tools::Nuclei;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lazy::{LazyLoadConfig, LoadStrategy};

    fn fp_with(stacks: Vec<TechStack>) -> TargetFingerprint {
        TargetFingerprint {
            target: "http://example.com".into(),
            stacks,
            server: None,
            open_ports: vec![],
            plugins: vec![],
            signals: vec![],
        }
    }

    #[test]
    fn wordpress_triggers_wpscan_and_ffuf() {
        let loader = LazyLoader::new(LazyLoadConfig {
            strategy: LoadStrategy::ExistingOnly,
            ..Default::default()
        });
        let planner = TacticalPlanner::new(&loader);
        let actions = planner.derive_actions(&fp_with(vec![TechStack::WordPress]));
        let tools: Vec<KaliTool> = actions.iter().map(|a| a.tool).collect();
        assert!(tools.contains(&KaliTool::Wpscan));
        assert!(tools.contains(&KaliTool::Ffuf));
        // wpscan should be the top-scored action.
        assert_eq!(actions[0].tool, KaliTool::Wpscan);
    }

    #[test]
    fn generic_target_still_gets_baseline() {
        let loader = LazyLoader::new(LazyLoadConfig::default());
        let planner = TacticalPlanner::new(&loader);
        let actions = planner.derive_actions(&fp_with(vec![TechStack::Generic]));
        assert!(actions.iter().any(|a| a.tool == KaliTool::Nuclei));
        assert!(actions.iter().any(|a| a.tool == KaliTool::Ffuf));
    }

    #[test]
    fn scoring_orders_descending() {
        let loader = LazyLoader::new(LazyLoadConfig::default());
        let planner = TacticalPlanner::new(&loader);
        let actions = planner.derive_actions(&fp_with(vec![TechStack::WordPress, TechStack::Php]));
        for w in actions.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    #[test]
    fn token_mapping() {
        assert_eq!(TechStack::from_token("WordPress"), Some(TechStack::WordPress));
        assert_eq!(TechStack::from_token("nginx"), Some(TechStack::Nginx));
        assert_eq!(TechStack::from_token("unclassified-thing"), None);
    }
}
