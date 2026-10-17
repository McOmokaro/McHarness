//! The agent loop: plan -> act -> observe, as one Whole.
//!
//! Tools are compiled-in organs invoked without handshake overhead.
//! The loop consults memory and Fusion natively; nothing is scripted
//! as a chain of external calls.

use crate::keyfarm::KeyFarm;
use crate::memory::Memory;
use crate::model::{ModelRequest, ModelRouter};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub text: String,
    pub domain: Option<String>,
    pub model_preference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActRecord {
    pub step: u32,
    pub observation: String,
}

pub struct AgentLoop {
    memory: Memory,
    router: ModelRouter,
    keyfarm: KeyFarm,
    max_steps: u32,
}

impl AgentLoop {
    pub fn new(memory: Memory, router: ModelRouter, keyfarm: KeyFarm) -> Self {
        Self {
            memory,
            router,
            keyfarm,
            max_steps: 16,
        }
    }

    /// Intent in, action out. No visible seams.
    pub async fn run(&mut self, intent: Intent) -> Result<String> {
        self.memory.working_append("intent", &intent.text)?;

        let context = self.memory.build_context();
        let mut acts: Vec<ActRecord> = Vec::new();

        for step in 1..=self.max_steps {
            let preferred = intent
                .model_preference
                .clone()
                .unwrap_or_else(|| "auto".to_string());

            // The Key Farm resolves the model + key before inference.
            let (model, key) = self.keyfarm.resolve(&preferred, intent.domain.as_deref())?;

            let request = ModelRequest {
                model,
                key,
                prompt: context.clone(),
                input: intent.text.clone(),
            };

            let observation = self.router.infer(request).await?;

            self.memory.working_append("observation", &observation)?;
            acts.push(ActRecord {
                step,
                observation: observation.clone(),
            });

            if observation.contains("<whole-complete>") {
                self.memory
                    .persistent_append("session_outcome", &observation)?;
                return Ok(observation);
            }
        }

        Ok(format!(
            "loop exhausted after {} steps; last observation: {}",
            self.max_steps,
            acts.last().map(|a| a.observation.as_str()).unwrap_or("none")
        ))
    }
}
