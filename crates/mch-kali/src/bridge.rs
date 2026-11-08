//! Bridge: wire the tactical Kali planner into the mch-cyber bug-bounty swarm.
//!
//! The bug-bounty framework deploys parallel agents per attack surface. This
//! module translates a [`TacticalPlan`] into concrete agent deployments plus
//! the attested tool outputs they should produce — "dynamic tool selection
//! based on the target's tech stack".

use crate::attest::{attest_output, AttestedOutput};
use crate::lazy::LazyLoader;
use crate::tactical::{PlannedAction, TacticalPlan};
use crate::tools::ToolCategory;
use mch_cyber::signing::ActionSigner;
use serde::{Deserialize, Serialize};

/// A single agent deployment derived from a planned action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDeployment {
    /// Logical agent name (e.g. "wpscan-hunter").
    pub agent: String,
    /// The Kali tool the agent drives.
    pub tool: String,
    /// Exact CLI args the agent will run.
    pub args: Vec<String>,
    /// Why the agent was chosen.
    pub rationale: String,
    /// Expected-yield score (higher = deploy sooner).
    pub priority: f64,
    /// Optional WordPress/CMS-specific wordlist, threaded into the agent.
    pub wordlist: Option<String>,
}

/// Derive the parallel agent deployments for a plan.
pub fn deployments_for(plan: &TacticalPlan) -> Vec<AgentDeployment> {
    plan.actions
        .iter()
        .map(|a: &PlannedAction| AgentDeployment {
            agent: format!("{}-agent", a.tool.binary()),
            tool: a.tool.binary().to_string(),
            args: a.args.clone(),
            rationale: a.rationale.clone(),
            priority: a.score,
            wordlist: extract_wordlist(a),
        })
        .collect()
}

fn extract_wordlist(action: &PlannedAction) -> Option<String> {
    action
        .args
        .iter()
        .position(|a| a == "-w")
        .and_then(|i| action.args.get(i + 1).cloned())
}

/// Execute the plan's actions and return attested outputs.
///
/// Only high-priority fingerprint/fuzzing actions are auto-run; exploitation
/// actions are returned as *planned* unless `allow_exploitation` is set, so
/// the mch-scope gate can review them first.
pub async fn execute_plan(
    loader: &LazyLoader,
    plan: &TacticalPlan,
    signer: &ActionSigner,
    allow_exploitation: bool,
) -> anyhow::Result<Vec<AttestedOutput>> {
    let mut attested = Vec::new();
    for action in &plan.actions {
        if action.category == ToolCategory::Exploitation && !allow_exploitation {
            continue;
        }
        let out = crate::tools::run_raw(loader, action.tool, &action.args, &plan.target).await?;
        attested.push(attest_output(signer, &out)?);
    }
    Ok(attested)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lazy::LazyLoadConfig;
    use crate::lazy::LoadStrategy;
    use crate::tactical::TechStack;
    use crate::tactical::TargetFingerprint;

    #[test]
    fn wordpress_plan_deploys_wpscan_agent() {
        let loader = LazyLoader::new(LazyLoadConfig {
            strategy: LoadStrategy::ExistingOnly,
            ..Default::default()
        });
        let planner = crate::tactical::TacticalPlanner::new(&loader);
        let fp = TargetFingerprint {
            target: "http://wp.example".into(),
            stacks: vec![TechStack::WordPress],
            server: None,
            open_ports: vec![],
            plugins: vec![],
            signals: vec![],
        };
        let plan = TacticalPlan {
            target: fp.target.clone(),
            fingerprint: fp.clone(),
            actions: planner.derive_actions(&fp),
        };
        let deps = deployments_for(&plan);
        assert!(deps.iter().any(|d| d.agent == "wpscan-agent"));
        // wpscan deployment must carry the WP wordlist for its sibling ffuf.
        assert!(deps.iter().any(|d| d.wordlist.is_some()));
    }
}
