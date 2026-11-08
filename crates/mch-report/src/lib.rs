//! mch-report — Morning Report compiler.
//!
//! Compiles communications, economic engine output, and hackathon results
//! into a formatted Markdown newspaper and TTS audio for the Overnight Agent.

pub mod compiler;
pub mod newspaper;
pub mod tts;

pub use compiler::{DataAggregator, ReportCompiler};
pub use newspaper::{NewspaperGenerator, SectionFormatter};
pub use tts::{TTSGenerator, VoiceSelector};