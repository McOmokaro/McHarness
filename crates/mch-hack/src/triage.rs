//! triage.rs — Project triage and rubric evaluation.
//!
//! Provides `ProjectTriage` for filtering/scoring projects and `RubricEvaluator`
//! for competition-specific rubric assessment.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A project to be evaluated for hackathon submission.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub repo_url: Option<String>,
    pub tech_stack: Vec<String>,
    pub team_size: u32,
    pub track: String,
    pub stage: ProjectStage,
    pub artifacts: Vec<Artifact>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStage {
    Idea,
    Prototype,
    Mvp,
    Polished,
    Production,
}

impl std::fmt::Display for ProjectStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStage::Idea => write!(f, "idea"),
            ProjectStage::Prototype => write!(f, "prototype"),
            ProjectStage::Mvp => write!(f, "mvp"),
            ProjectStage::Polished => write!(f, "polished"),
            ProjectStage::Production => write!(f, "production"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Artifact {
    pub name: String,
    pub kind: ArtifactKind,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    SourceCode,
    Binary,
    Document,
    Image,
    Video,
    Deck,
    Other,
}

/// Evaluation result from triage.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Evaluation {
    pub project_id: Uuid,
    pub overall_score: f32,
    pub dimension_scores: HashMap<String, f32>,
    pub verdict: Verdict,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub recommendations: Vec<String>,
    pub rubric_match: RubricMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    StrongFit,
    GoodFit,
    MarginalFit,
    PoorFit,
    Reject,
}

impl Verdict {
    pub fn from_score(score: f32) -> Self {
        match score {
            s if s >= 0.85 => Verdict::StrongFit,
            s if s >= 0.70 => Verdict::GoodFit,
            s if s >= 0.50 => Verdict::MarginalFit,
            s if s >= 0.30 => Verdict::PoorFit,
            _ => Verdict::Reject,
        }
    }
}

/// Match against a specific competition rubric.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RubricMatch {
    pub rubric_id: String,
    pub rubric_name: String,
    pub criterion_scores: HashMap<String, CriterionScore>,
    pub weighted_total: f32,
    pub max_possible: f32,
    pub percentile: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CriterionScore {
    pub name: String,
    pub raw_score: f32,
    pub weighted_score: f32,
    pub max_score: f32,
    pub weight: f32,
    pub evidence: Vec<String>,
}

/// Competition rubric definition.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Rubric {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub version: String,
    pub criteria: Vec<RubricCriterion>,
    pub scoring: ScoringMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RubricCriterion {
    pub id: String,
    pub name: String,
    pub description: String,
    pub weight: f32,
    pub max_score: u32,
    pub evaluation_guidance: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScoringMethod {
    WeightedSum,
    GeometricMean,
    MinimumThreshold,
    Custom,
}

/// Project triage engine — filters and ranks projects for hackathon fit.
pub struct ProjectTriage {
    rubrics: HashMap<String, Rubric>,
    min_score_threshold: f32,
}

impl ProjectTriage {
    pub fn new() -> Self {
        let mut triage = Self {
            rubrics: HashMap::new(),
            min_score_threshold: 0.4,
        };
        triage.register_builtin_rubrics();
        triage
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.min_score_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Register a custom rubric.
    pub fn register_rubric(&mut self, rubric: Rubric) {
        self.rubrics.insert(rubric.id.clone(), rubric);
    }

    /// Evaluate a project against all registered rubrics, return best match.
    pub fn evaluate(&self, project: &Project) -> Evaluation {
        let evaluator = RubricEvaluator::new(&self.rubrics);
        evaluator.evaluate_best(project)
    }

    /// Evaluate against a specific rubric.
    pub fn evaluate_against(&self, project: &Project, rubric_id: &str) -> Option<Evaluation> {
        let evaluator = RubricEvaluator::new(&self.rubrics);
        evaluator.evaluate(project, rubric_id)
    }

    /// Filter projects by minimum score.
    pub fn filter(&self, projects: &[Project]) -> Vec<Project> {
        projects
            .iter()
            .filter(|p| self.evaluate(p).overall_score >= self.min_score_threshold)
            .cloned()
            .collect()
    }

    /// Rank projects by score (descending).
    pub fn rank(&self, projects: &[Project]) -> Vec<(Project, f32)> {
        let mut scored: Vec<_> = projects
            .iter()
            .map(|p| (p.clone(), self.evaluate(p).overall_score))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored
    }

    fn register_builtin_rubrics(&mut self) {
        // hackit-ai style rubric: 3-min pitch, TUI, competition rubrics
        self.register_rubric(Rubric {
            id: "hackit-ai".into(),
            name: "hackit.ai Competition Rubric".into(),
            platform: "hackit.ai".into(),
            version: "1.0".into(),
            scoring: ScoringMethod::WeightedSum,
            criteria: vec![
                RubricCriterion {
                    id: "technical_innovation".into(),
                    name: "Technical Innovation".into(),
                    description: "Novelty of approach, algorithmic cleverness, architecture".into(),
                    weight: 0.25,
                    max_score: 10,
                    evaluation_guidance: "Look for unique technical approaches, not just standard CRUD".into(),
                },
                RubricCriterion {
                    id: "pitch_quality".into(),
                    name: "3-Minute Pitch Quality".into(),
                    description: "Clarity, storytelling, demo effectiveness, time management".into(),
                    weight: 0.20,
                    max_score: 10,
                    evaluation_guidance: "Pitch must be under 3 minutes, clear problem-solution narrative".into(),
                },
                RubricCriterion {
                    id: "implementation_completeness".into(),
                    name: "Implementation Completeness".into(),
                    description: "Working demo, test coverage, deployment readiness".into(),
                    weight: 0.20,
                    max_score: 10,
                    evaluation_guidance: "Must have working code, not just slides. TUI/CLI preferred".into(),
                },
                RubricCriterion {
                    id: "market_potential".into(),
                    name: "Market Potential".into(),
                    description: "Addressable market, business model clarity, go-to-market".into(),
                    weight: 0.15,
                    max_score: 10,
                    evaluation_guidance: "Real problem, viable path to users/revenue".into(),
                },
                RubricCriterion {
                    id: "design_ux".into(),
                    name: "Design & UX".into(),
                    description: "Visual polish, usability, accessibility, delight".into(),
                    weight: 0.10,
                    max_score: 10,
                    evaluation_guidance: "Even CLI/TUI should have thoughtful UX".into(),
                },
                RubricCriterion {
                    id: "team_execution".into(),
                    name: "Team Execution".into(),
                    description: "Velocity, scope management, collaboration evidence".into(),
                    weight: 0.10,
                    max_score: 10,
                    evaluation_guidance: "Git history, issue tracking, clear roles".into(),
                },
            ],
        });

        // 0xClaw style: autonomous competitor rubric
        self.register_rubric(Rubric {
            id: "0xclaw".into(),
            name: "0xClaw Autonomous Agent Rubric".into(),
            platform: "0xclaw".into(),
            version: "1.0".into(),
            scoring: ScoringMethod::WeightedSum,
            criteria: vec![
                RubricCriterion {
                    id: "autonomy_level".into(),
                    name: "Autonomy Level".into(),
                    description: "Degree of human-in-the-loop vs fully autonomous operation".into(),
                    weight: 0.30,
                    max_score: 10,
                    evaluation_guidance: "Higher score for less human intervention needed".into(),
                },
                RubricCriterion {
                    id: "tool_use_sophistication".into(),
                    name: "Tool Use Sophistication".into(),
                    description: "Breadth and depth of tool integration, chaining, error recovery".into(),
                    weight: 0.25,
                    max_score: 10,
                    evaluation_guidance: "Multiple tools, parallel execution, self-correction".into(),
                },
                RubricCriterion {
                    id: "task_completion_rate".into(),
                    name: "Task Completion Rate".into(),
                    description: "Percentage of assigned tasks completed successfully".into(),
                    weight: 0.20,
                    max_score: 10,
                    evaluation_guidance: "Track record on benchmark suites".into(),
                },
                RubricCriterion {
                    id: "adaptability".into(),
                    name: "Adaptability".into(),
                    description: "Handles novel tasks, recovers from failures, learns".into(),
                    weight: 0.15,
                    max_score: 10,
                    evaluation_guidance: "Out-of-distribution generalization".into(),
                },
                RubricCriterion {
                    id: "resource_efficiency".into(),
                    name: "Resource Efficiency".into(),
                    description: "Token/cost optimization, latency, parallelization".into(),
                    weight: 0.10,
                    max_score: 10,
                    evaluation_guidance: "Pareto frontier of quality vs cost".into(),
                },
            ],
        });

        // Generic web3/blockchain hackathon rubric
        self.register_rubric(Rubric {
            id: "web3-general".into(),
            name: "General Web3 Hackathon Rubric".into(),
            platform: "generic".into(),
            version: "1.0".into(),
            scoring: ScoringMethod::WeightedSum,
            criteria: vec![
                RubricCriterion {
                    id: "onchain_innovation".into(),
                    name: "On-Chain Innovation".into(),
                    description: "Novel smart contract patterns, new primitives, composability".into(),
                    weight: 0.30,
                    max_score: 10,
                    evaluation_guidance: "Not just another ERC20/721/1155 clone".into(),
                },
                RubricCriterion {
                    id: "security_auditability".into(),
                    name: "Security & Auditability".into(),
                    description: "Code quality, test coverage, formal verification readiness".into(),
                    weight: 0.25,
                    max_score: 10,
                    evaluation_guidance: "Tests, invariants, fuzzing, audit reports".into(),
                },
                RubricCriterion {
                    id: "user_experience".into(),
                    name: "User Experience".into(),
                    description: "Wallet UX, gas optimization, account abstraction, onboarding".into(),
                    weight: 0.20,
                    max_score: 10,
                    evaluation_guidance: "Abstract away complexity for end users".into(),
                },
                RubricCriterion {
                    id: "ecosystem_integration".into(),
                    name: "Ecosystem Integration".into(),
                    description: "Composes with existing protocols, standards compliance".into(),
                    weight: 0.15,
                    max_score: 10,
                    evaluation_guidance: "ERC/EIP standards, cross-protocol integration".into(),
                },
                RubricCriterion {
                    id: "sustainability".into(),
                    name: "Sustainability".into(),
                    description: "Tokenomics, incentive alignment, long-term viability".into(),
                    weight: 0.10,
                    max_score: 10,
                    evaluation_guidance: "Not a ponzi, real value accrual mechanism".into(),
                },
            ],
        });
    }
}

impl Default for ProjectTriage {
    fn default() -> Self {
        Self::new()
    }
}

/// Rubric evaluator — scores projects against specific competition rubrics.
pub struct RubricEvaluator {
    rubrics: HashMap<String, Rubric>,
}

impl RubricEvaluator {
    pub fn new(rubrics: &HashMap<String, Rubric>) -> Self {
        Self {
            rubrics: rubrics.clone(),
        }
    }

    /// Evaluate against all rubrics, return best match.
    pub fn evaluate_best(&self, project: &Project) -> Evaluation {
        let mut best: Option<Evaluation> = None;
        for rubric in self.rubrics.values() {
            if let Some(eval) = self.evaluate(project, &rubric.id) {
                if best.is_none() || eval.overall_score > best.as_ref().unwrap().overall_score {
                    best = Some(eval);
                }
            }
        }
        best.unwrap_or_else(|| Evaluation {
            project_id: project.id,
            overall_score: 0.0,
            dimension_scores: HashMap::new(),
            verdict: Verdict::Reject,
            strengths: vec![],
            weaknesses: vec!["no matching rubric".into()],
            recommendations: vec!["register a relevant rubric".into()],
            rubric_match: RubricMatch {
                rubric_id: "none".into(),
                rubric_name: "none".into(),
                criterion_scores: HashMap::new(),
                weighted_total: 0.0,
                max_possible: 0.0,
                percentile: 0.0,
            },
        })
    }

    /// Evaluate against a specific rubric.
    pub fn evaluate(&self, project: &Project, rubric_id: &str) -> Option<Evaluation> {
        let rubric = self.rubrics.get(rubric_id)?;
        let mut criterion_scores = HashMap::new();
        let mut weighted_total = 0.0;
        let mut max_possible = 0.0;
        let mut strengths = Vec::new();
        let mut weaknesses = Vec::new();

        for criterion in &rubric.criteria {
            let raw = self.score_criterion(project, criterion);
            let weighted = raw * criterion.weight;
            weighted_total += weighted;
            max_possible += criterion.max_score as f32 * criterion.weight;

            criterion_scores.insert(criterion.id.clone(), CriterionScore {
                name: criterion.name.clone(),
                raw_score: raw,
                weighted_score: weighted,
                max_score: criterion.max_score as f32,
                weight: criterion.weight,
                evidence: self.gather_evidence(project, criterion),
            });

            if raw >= criterion.max_score as f32 * 0.7 {
                strengths.push(format!("Strong on {}", criterion.name));
            } else if raw <= criterion.max_score as f32 * 0.3 {
                weaknesses.push(format!("Weak on {}", criterion.name));
            }
        }

        let overall_score = if max_possible > 0.0 { weighted_total / max_possible } else { 0.0 };
        let verdict = Verdict::from_score(overall_score);

        let mut recommendations = Vec::new();
        for criterion in &rubric.criteria {
            let score = criterion_scores.get(&criterion.id).map(|cs| cs.raw_score).unwrap_or(0.0);
            if score < criterion.max_score as f32 * 0.5 {
                recommendations.push(format!("Improve {}: {}", criterion.name, criterion.evaluation_guidance));
            }
        }

        Some(Evaluation {
            project_id: project.id,
            overall_score,
            dimension_scores: criterion_scores.iter()
                .map(|(k, v)| (k.clone(), v.raw_score))
                .collect(),
            verdict,
            strengths,
            weaknesses,
            recommendations,
            rubric_match: RubricMatch {
                rubric_id: rubric.id.clone(),
                rubric_name: rubric.name.clone(),
                criterion_scores,
                weighted_total,
                max_possible,
                percentile: self.estimate_percentile(overall_score, rubric),
            },
        })
    }

    fn score_criterion(&self, project: &Project, criterion: &RubricCriterion) -> f32 {
        // Heuristic scoring based on project attributes
        match criterion.id.as_str() {
            "technical_innovation" | "onchain_innovation" => self.score_innovation(project),
            "pitch_quality" => self.score_pitch_readiness(project),
            "implementation_completeness" => self.score_completeness(project),
            "market_potential" => self.score_market(project),
            "design_ux" => self.score_design(project),
            "team_execution" => self.score_execution(project),
            "autonomy_level" => self.score_autonomy(project),
            "tool_use_sophistication" => self.score_tool_use(project),
            "task_completion_rate" => self.score_completion(project),
            "adaptability" => self.score_adaptability(project),
            "resource_efficiency" => self.score_efficiency(project),
            "security_auditability" => self.score_security(project),
            "user_experience" => self.score_ux(project),
            "ecosystem_integration" => self.score_integration(project),
            "sustainability" => self.score_sustainability(project),
            _ => 5.0, // neutral default
        }
    }

    fn score_innovation(&self, project: &Project) -> f32 {
        let mut score = 5.0;
        // Bonus for novel tech stack combinations
        if project.tech_stack.len() >= 3 {
            score += 1.0;
        }
        // Bonus for cutting-edge keywords
        let innovative = ["zksnark", "zksnark", "fhe", "tee", "avs", "eigenlayer", "intent", "account abstraction", "erc4337", "rollup", "validium", "daza", "mopro", "risc0", "sp1", "cairo", "noir", "circom"];
        for tech in &project.tech_stack {
            if innovative.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 0.5;
            }
        }
        score.min(10.0)
    }

    fn score_pitch_readiness(&self, project: &Project) -> f32 {
        let mut score = 4.0;
        if project.artifacts.iter().any(|a| a.kind == ArtifactKind::Deck) {
            score += 3.0;
        }
        if project.description.len() > 200 {
            score += 1.5;
        }
        if project.stage >= ProjectStage::Mvp {
            score += 1.5;
        }
        score.min(10.0)
    }

    fn score_completeness(&self, project: &Project) -> f32 {
        match project.stage {
            ProjectStage::Idea => 2.0,
            ProjectStage::Prototype => 4.0,
            ProjectStage::Mvp => 6.0,
            ProjectStage::Polished => 8.5,
            ProjectStage::Production => 10.0,
        }
    }

    fn score_market(&self, project: &Project) -> f32 {
        let mut score = 5.0;
        if project.description.to_lowercase().contains("market") || project.description.to_lowercase().contains("user") {
            score += 1.0;
        }
        if project.team_size >= 2 {
            score += 1.0;
        }
        score.min(10.0)
    }

    fn score_design(&self, project: &Project) -> f32 {
        let mut score = 5.0;
        if project.artifacts.iter().any(|a| a.kind == ArtifactKind::Image || a.kind == ArtifactKind::Video) {
            score += 2.0;
        }
        if project.tech_stack.iter().any(|t| t.to_lowercase().contains("ui") || t.to_lowercase().contains("ux") || t.to_lowercase().contains("design")) {
            score += 1.0;
        }
        score.min(10.0)
    }

    fn score_execution(&self, project: &Project) -> f32 {
        let mut score = 5.0;
        if project.repo_url.is_some() {
            score += 2.0;
        }
        if project.artifacts.iter().any(|a| a.kind == ArtifactKind::SourceCode) {
            score += 1.5;
        }
        if project.team_size >= 3 {
            score += 1.0;
        }
        score.min(10.0)
    }

    fn score_autonomy(&self, project: &Project) -> f32 {
        let mut score = 4.0;
        let autonomous_keywords = ["autonomous", "agent", "llm", "gpt", "claude", "auto", "self-heal", "self-correct"];
        for tech in &project.tech_stack {
            if autonomous_keywords.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 1.5;
            }
        }
        if project.description.to_lowercase().contains("autonomous") {
            score += 1.0;
        }
        score.min(10.0)
    }

    fn score_tool_use(&self, project: &Project) -> f32 {
        let mut score = 4.0;
        let tool_keywords = ["mcp", "tool", "function calling", "api", "integration", "plugin", "extension"];
        for tech in &project.tech_stack {
            if tool_keywords.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 1.0;
            }
        }
        score.min(10.0)
    }

    fn score_completion(&self, project: &Project) -> f32 {
        match project.stage {
            ProjectStage::Idea => 1.0,
            ProjectStage::Prototype => 3.0,
            ProjectStage::Mvp => 5.5,
            ProjectStage::Polished => 8.0,
            ProjectStage::Production => 10.0,
        }
    }

    fn score_adaptability(&self, project: &Project) -> f32 {
        let mut score = 5.0;
        if project.tech_stack.len() > 4 {
            score += 1.0;
        }
        if project.description.to_lowercase().contains("general") || project.description.to_lowercase().contains("flexible") {
            score += 1.0;
        }
        score.min(10.0)
    }

    fn score_efficiency(&self, project: &Project) -> f32 {
        let mut score = 5.0;
        let eff_keywords = ["optimiz", "fast", "efficient", "low latency", "parallel", "async", "streaming", "batch"];
        for tech in &project.tech_stack {
            if eff_keywords.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 0.8;
            }
        }
        score.min(10.0)
    }

    fn score_security(&self, project: &Project) -> f32 {
        let mut score = 4.0;
        let sec_keywords = ["audit", "test", "fuzz", "invariant", "formal", "verif", "secure", "slither", "echidna", "foundry"];
        for tech in &project.tech_stack {
            if sec_keywords.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 1.2;
            }
        }
        if project.artifacts.iter().any(|a| a.kind == ArtifactKind::SourceCode) {
            score += 1.0;
        }
        score.min(10.0)
    }

    fn score_ux(&self, project: &Project) -> f32 {
        let mut score = 4.0;
        let ux_keywords = ["account abstraction", "erc4337", "wallet", "onboard", "gasless", "sponsor", "passkey", "biometric"];
        for tech in &project.tech_stack {
            if ux_keywords.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 1.5;
            }
        }
        score.min(10.0)
    }

    fn score_integration(&self, project: &Project) -> f32 {
        let mut score = 5.0;
        let int_keywords = ["erc", "eip", "standard", "compos", "integrat", "protocol", "defi", "dex", "lending"];
        for tech in &project.tech_stack {
            if int_keywords.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 0.8;
            }
        }
        score.min(10.0)
    }

    fn score_sustainability(&self, project: &Project) -> f32 {
        let mut score = 4.0;
        let sus_keywords = ["tokenomic", "incentive", "revenue", "fee", "value accrual", "buyback", "stake", "governance"];
        for tech in &project.tech_stack {
            if sus_keywords.iter().any(|k| tech.to_lowercase().contains(k)) {
                score += 1.0;
            }
        }
        score.min(10.0)
    }

    fn gather_evidence(&self, project: &Project, criterion: &RubricCriterion) -> Vec<String> {
        let mut evidence = Vec::new();
        evidence.push(format!("Stage: {}", project.stage));
        evidence.push(format!("Tech stack: {}", project.tech_stack.join(", ")));
        if let Some(url) = &project.repo_url {
            evidence.push(format!("Repo: {}", url));
        }
        evidence.push(format!("Artifacts: {}", project.artifacts.len()));
        evidence
    }

    fn estimate_percentile(&self, score: f32, rubric: &Rubric) -> f32 {
        // Simplified: assume normal distribution around 0.5 mean
        // In practice, would use historical submission data
        let z = (score - 0.5) / 0.2;
        let percentile = 0.5 * (1.0 + libm::erf(z / 1.4142));
        (percentile * 100.0).clamp(0.0, 100.0)
    }
}

impl Default for RubricEvaluator {
    fn default() -> Self {
        Self::new(&HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_project() -> Project {
        Project {
            id: Uuid::new_v4(),
            name: "Autonomous DeFi Agent".into(),
            description: "An autonomous agent that optimizes DeFi yields across protocols using MEV protection and intent-based execution".into(),
            repo_url: Some("https://github.com/example/agent".into()),
            tech_stack: vec!["rust".into(), "ethers".into(), "foundry".into(), "erc4337".into(), "account abstraction".into()],
            team_size: 3,
            track: "defi".into(),
            stage: ProjectStage::Mvp,
            artifacts: vec![
                Artifact { name: "src".into(), kind: ArtifactKind::SourceCode, path: "src/".into(), size_bytes: 50000 },
                Artifact { name: "pitch.pdf".into(), kind: ArtifactKind::Deck, path: "pitch.pdf".into(), size_bytes: 200000 },
            ],
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn triage_constructs() {
        let _triage = ProjectTriage::new();
    }

    #[test]
    fn evaluate_project() {
        let triage = ProjectTriage::new();
        let project = sample_project();
        let eval = triage.evaluate(&project);
        assert!(eval.overall_score > 0.0);
        assert!(eval.overall_score <= 1.0);
        assert!(!eval.rubric_match.rubric_id.is_empty());
    }

    #[test]
    fn verdict_from_score() {
        assert_eq!(Verdict::from_score(0.9), Verdict::StrongFit);
        assert_eq!(Verdict::from_score(0.75), Verdict::GoodFit);
        assert_eq!(Verdict::from_score(0.55), Verdict::MarginalFit);
        assert_eq!(Verdict::from_score(0.35), Verdict::PoorFit);
        assert_eq!(Verdict::from_score(0.1), Verdict::Reject);
    }

    #[test]
    fn filter_projects() {
        let triage = ProjectTriage::new().with_threshold(0.5);
        let p1 = sample_project();
        let mut p2 = sample_project();
        p2.name = "Low Effort".into();
        p2.stage = ProjectStage::Idea;
        p2.tech_stack = vec!["python".into()];
        p2.artifacts.clear();

        let filtered = triage.filter(&[p1.clone(), p2]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Autonomous DeFi Agent");
    }

    #[test]
    fn rank_projects() {
        let triage = ProjectTriage::new();
        let p1 = sample_project();
        let mut p2 = sample_project();
        p2.name = "Better Project".into();
        p2.stage = ProjectStage::Production;
        p2.tech_stack.push("formal verification".into());

        let ranked = triage.rank(&[p1, p2]);
        assert_eq!(ranked.len(), 2);
        assert!(ranked[0].1 >= ranked[1].1);
    }

    #[test]
    fn rubric_evaluator_specific() {
        let triage = ProjectTriage::new();
        let project = sample_project();
        let eval = triage.evaluate_against(&project, "hackit-ai").unwrap();
        assert_eq!(eval.rubric_match.rubric_id, "hackit-ai");
    }

    #[test]
    fn criterion_scoring() {
        let evaluator = RubricEvaluator::new(&HashMap::new());
        let project = sample_project();

        // Test innovation scoring
        let criterion = RubricCriterion {
            id: "technical_innovation".into(),
            name: "Technical Innovation".into(),
            description: "".into(),
            weight: 1.0,
            max_score: 10,
            evaluation_guidance: "".into(),
        };
        let score = evaluator.score_criterion(&project, &criterion);
        assert!(score > 5.0); // Should get bonus for innovative stack
    }
}