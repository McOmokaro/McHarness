//! mch-dsh — the DeepSeek Harness Forge integration, unified.
//!
//! Re-exports the four DSH sub-crates so the Cordis kernel, the BYOK LLM
//! service, Super-Agent routing, and the dynamic package runtime are reachable
//! from one crate. Forged from `deepseek-ai/deepseek-harness`.

pub use mch_dsh_agent as agent;
pub use mch_dsh_cordis as cordis;
pub use mch_dsh_dynamic as dynamic;
pub use mch_dsh_llm as llm;
