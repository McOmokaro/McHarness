//! v0.0.1 skeleton demonstration: the Whole boots.

use mcharness_core::{keyfarm::{FarmKey, KeyFarm}, loop_::{AgentLoop, Intent}, memory::Memory, model::ModelRouter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let memory = Memory::open("mcharness.db")?;
    let router = ModelRouter;
    let mut farm = KeyFarm::new();
    farm.seed_leaderboard(
        "general",
        vec!["local:qwen2.5".into(), "local:llama3.1".into()],
    );
    farm.add_key(FarmKey {
        id: "local-ollama".into(),
        provider: "ollama".into(),
        models: vec!["local:qwen2.5".into(), "local:llama3.1".into()],
        usage_count: 0,
        rate_limit_pct: 0.0,
        paused: false,
    });

    let mut agent = AgentLoop::new(memory, router, farm);
    let intent = Intent {
        text: "Summarize the McHarness architecture in one sentence.".into(),
        domain: Some("general".into()),
        model_preference: Some("auto".into()),
    };

    let result = agent.run(intent).await?;
    println!("{result}");
    Ok(())
}
