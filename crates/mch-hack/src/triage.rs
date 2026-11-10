//! Project triage and rubric evaluation.

use serde::{Deserialize, Serialize};

/// Delivery stage of a candidate project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStage {
    Idea,
    Prototype,
    Mvp,
    Polished,
}

/// A candidate project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub stage: ProjectStage,
    pub features: Vec<String>,
    pub tech_stack: Vec<String>,
    pub has_demo: bool,
    pub has_pitch: bool,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            stage: ProjectStage::Idea,
            features: Vec::new(),
            tech_stack: Vec::new(),
            has_demo: false,
            has_pitch: false,
        }
    }
}

/// A single rubric criterion and its score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RubricScore {
    pub criterion: String,
    pub score: f64,
    pub max: f64,
    pub note: String,
}

/// The outcome of evaluating a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evaluation {
    pub project_id: String,
    pub total: f64,
    pub max_total: f64,
    pub breakdown: Vec<RubricScore>,
    pub recommendation: String,
}

impl Evaluation {
    pub fn normalized(&self) -> f64 {
        if self.max_total <= 0.0 {
            0.0
        } else {
            self.total / self.max_total
        }
    }
}

/// Scores a single rubric criterion (0..max).
pub struct RubricEvaluator {
    criteria: Vec<(String, f64)>,
}

impl RubricEvaluator {
    pub fn new() -> Self {
        Self {
            criteria: vec![
                ("innovation".into(), 10.0),
                ("technical_depth".into(), 10.0),
                ("completeness".into(), 10.0),
                ("impact".into(), 10.0),
                ("presentation".into(), 10.0),
            ],
        }
    }

    pub fn criteria(&self) -> &[(String, f64)] {
        &self.criteria
    }

    pub fn score(&self, project: &Project, criterion: &str) -> RubricScore {
        let max = self
            .criteria
            .iter()
            .find(|(c, _)| c == criterion)
            .map(|(_, m)| *m)
            .unwrap_or(10.0);
        let score = match criterion {
            "innovation" => (project.features.len() as f64).min(max),
            "technical_depth" => (project.tech_stack.len() as f64 * 2.0).min(max),
            "completeness" => match project.stage {
                ProjectStage::Idea => 2.0,
                ProjectStage::Prototype => 5.0,
                ProjectStage::Mvp => 8.0,
                ProjectStage::Polished => 10.0,
            },
            "impact" => if project.has_demo { 7.0 } else { 4.0 },
            "presentation" => if project.has_pitch { max } else { 3.0 },
            _ => 0.0,
        };
        RubricScore {
            criterion: criterion.to_string(),
            score,
            max,
            note: String::new(),
        }
    }
}

impl Default for RubricEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluates projects against the rubric.
pub struct ProjectTriage {
    evaluator: RubricEvaluator,
}

impl ProjectTriage {
    pub fn new() -> Self {
        Self { evaluator: RubricEvaluator::new() }
    }

    pub fn evaluate(&self, project: &Project) -> Evaluation {
        let mut breakdown = Vec::new();
        let mut total = 0.0;
        let mut max_total = 0.0;
        for (criterion, _) in self.evaluator.criteria() {
            let s = self.evaluator.score(project, criterion);
            total += s.score;
            max_total += s.max;
            breakdown.push(s);
        }
        let normalized = if max_total > 0.0 { total / max_total } else { 0.0 };
        let recommendation = if normalized >= 0.8 {
            "submit — strong candidate".to_string()
        } else if normalized >= 0.55 {
            "polish then submit".to_string()
        } else {
            "hold — needs work".to_string()
        };
        Evaluation {
            project_id: project.id.clone(),
            total,
            max_total,
            breakdown,
            recommendation,
        }
    }
}

impl Default for ProjectTriage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polished_project_scores_higher_than_idea() {
        let triage = ProjectTriage::new();
        let mut p = Project::new("x");
        p.stage = ProjectStage::Polished;
        p.has_demo = true;
        p.has_pitch = true;
        p.tech_stack = vec!["rust".into(), "tokio".into(), "sqlite".into()];
        p.features = vec!["a".into(), "b".into(), "c".into()];
        let good = triage.evaluate(&p);

        let idea = triage.evaluate(&Project::new("y"));
        assert!(good.normalized() > idea.normalized());
    }
}
