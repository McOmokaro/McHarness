//! TTS audio generation for Morning Report.
//!
//! Converts compiled newspaper text into spoken audio using configurable
//! voice selection and synthesis backends.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Generates TTS audio from report text.
#[derive(Debug, Default)]
pub struct TTSGenerator {
    /// Voice selector for choosing synthesis voice
    pub voice_selector: VoiceSelector,
    /// Output directory for audio files
    pub output_dir: PathBuf,
    /// Speech rate multiplier (0.5 - 2.0)
    pub rate: f32,
    /// Audio format
    pub format: AudioFormat,
    /// Maximum text chunk size for synthesis
    pub max_chunk_chars: usize,
}

impl TTSGenerator {
    /// Create a new generator with defaults.
    pub fn new() -> Self {
        Self {
            output_dir: PathBuf::from("./reports/audio"),
            rate: 1.0,
            format: AudioFormat::Mp3,
            max_chunk_chars: 4000,
            ..Default::default()
        }
    }

    /// Set output directory.
    pub fn with_output_dir(mut self, dir: PathBuf) -> Self {
        self.output_dir = dir;
        self
    }

    /// Set speech rate (0.5 - 2.0).
    pub fn with_rate(mut self, rate: f32) -> Self {
        self.rate = rate.clamp(0.5, 2.0);
        self
    }

    /// Set audio format.
    pub fn with_format(mut self, format: AudioFormat) -> Self {
        self.format = format;
        self
    }

    /// Set voice selector.
    pub fn with_voice_selector(mut self, selector: VoiceSelector) -> Self {
        self.voice_selector = selector;
        self
    }

    /// Generate audio from newspaper text.
    pub async fn generate(&self, text: &str, report_date: &chrono::DateTime<chrono::Utc>) -> Result<AudioOutput> {
        // Ensure output directory exists
        tokio::fs::create_dir_all(&self.output_dir).await?;

        let voice = self.voice_selector.select(text)?;
        let chunks = self.chunk_text(text);

        let mut audio_segments = Vec::new();
        let mut total_duration = Duration::ZERO;

        for (i, chunk) in chunks.iter().enumerate() {
            let segment_path = self.output_dir.join(format!(
                "morning_report_{}_{:03}.{}",
                report_date.format("%Y%m%d"),
                i,
                self.format.extension()
            ));

            let duration = self.synthesize_chunk(chunk, &voice, &segment_path).await?;
            total_duration += duration;
            audio_segments.push(AudioSegment {
                path: segment_path,
                text: chunk.clone(),
                duration,
                voice: voice.clone(),
            });
        }

        // Concatenate segments if multiple
        let final_path = if audio_segments.len() > 1 {
            self.concatenate_segments(&audio_segments, report_date).await?
        } else {
            audio_segments[0].path.clone()
        };

        Ok(AudioOutput {
            path: final_path,
            format: self.format,
            duration: total_duration,
            segments: audio_segments,
            voice: voice,
        })
    }

    /// Generate audio and return just the file path.
    pub async fn generate_to_file(&self, text: &str, report_date: &chrono::DateTime<chrono::Utc>) -> Result<PathBuf> {
        self.generate(text, report_date).await.map(|o| o.path)
    }

    fn chunk_text(&self, text: &str) -> Vec<String> {
        if text.len() <= self.max_chunk_chars {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for paragraph in text.split("\n\n") {
            if current.len() + paragraph.len() + 2 > self.max_chunk_chars && !current.is_empty() {
                chunks.push(current.trim().to_string());
                current.clear();
            }
            if !current.is_empty() {
                current.push_str("\n\n");
            }
            current.push_str(paragraph);
        }

        if !current.is_empty() {
            chunks.push(current.trim().to_string());
        }

        chunks
    }

    async fn synthesize_chunk(&self, text: &str, voice: &Voice, output_path: &PathBuf) -> Result<Duration> {
        // This is a stub implementation - in production would call actual TTS backend
        // For now, create a placeholder file and return estimated duration
        tokio::fs::write(output_path, format!("TTS_PLACEHOLDER: {}", text.len())).await?;

        // Estimate: ~150 words per minute, ~5 chars per word
        let word_count = text.split_whitespace().count();
        let estimated_secs = (word_count as f64 / 150.0 * 60.0 / self.rate as f64) as u64;

        Ok(Duration::from_secs(estimated_secs.max(1)))
    }

    async fn concatenate_segments(&self, segments: &[AudioSegment], report_date: &chrono::DateTime<chrono::Utc>) -> Result<PathBuf> {
        let final_path = self.output_dir.join(format!(
            "morning_report_{}.{}",
            report_date.format("%Y%m%d"),
            self.format.extension()
        ));

        // In production, would use ffmpeg or similar to concatenate
        // For now, create a manifest file
        let manifest = segments
            .iter()
            .map(|s| s.path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        tokio::fs::write(&final_path, format!("CONCATENATED:\n{}", manifest)).await?;

        Ok(final_path)
    }
}

/// Selects appropriate voice for TTS synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSelector {
    /// Preferred voice name
    pub preferred_voice: Option<String>,
    /// Language code (e.g., "en-US", "en-GB")
    pub language: String,
    /// Gender preference
    pub gender: Option<VoiceGender>,
    /// Voice style/tone
    pub style: Option<VoiceStyle>,
    /// Fallback voices in order
    pub fallbacks: Vec<String>,
}

impl Default for VoiceSelector {
    fn default() -> Self {
        Self {
            preferred_voice: None,
            language: "en-US".to_string(),
            gender: Some(VoiceGender::Neutral),
            style: Some(VoiceStyle::News),
            fallbacks: vec!["default".to_string()],
        }
    }
}

impl VoiceSelector {
    /// Create a new voice selector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set preferred voice.
    pub fn with_voice(mut self, voice: String) -> Self {
        self.preferred_voice = Some(voice);
        self
    }

    /// Set language.
    pub fn with_language(mut self, lang: String) -> Self {
        self.language = lang;
        self
    }

    /// Set gender preference.
    pub fn with_gender(mut self, gender: VoiceGender) -> Self {
        self.gender = Some(gender);
        self
    }

    /// Set voice style.
    pub fn with_style(mut self, style: VoiceStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// Select the best voice for the given text.
    pub fn select(&self, _text: &str) -> Result<Voice> {
        // In production, would query TTS backend for available voices
        // and select best match based on preferences
        Ok(Voice {
            name: self.preferred_voice.clone().unwrap_or_else(|| "default".to_string()),
            language: self.language.clone(),
            gender: self.gender.clone().unwrap_or(VoiceGender::Neutral),
            style: self.style.clone().unwrap_or(VoiceStyle::News),
            sample_rate: 24000,
        })
    }

    /// List available voices (stub).
    pub async fn list_available(&self) -> Result<Vec<Voice>> {
        // In production, would query TTS backend
        Ok(vec![
            Voice {
                name: "nova".to_string(),
                language: "en-US".to_string(),
                gender: VoiceGender::Female,
                style: VoiceStyle::News,
                sample_rate: 24000,
            },
            Voice {
                name: "echo".to_string(),
                language: "en-US".to_string(),
                gender: VoiceGender::Male,
                style: VoiceStyle::News,
                sample_rate: 24000,
            },
            Voice {
                name: "onyx".to_string(),
                language: "en-US".to_string(),
                gender: VoiceGender::Male,
                style: VoiceStyle::Conversational,
                sample_rate: 24000,
            },
        ])
    }
}

/// Selected voice for synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub name: String,
    pub language: String,
    pub gender: VoiceGender,
    pub style: VoiceStyle,
    pub sample_rate: u32,
}

/// Voice gender options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceGender {
    Male,
    Female,
    Neutral,
}

/// Voice style/tone options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceStyle {
    News,
    Conversational,
    Narration,
    Formal,
    Friendly,
}

/// Audio output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Mp3,
    Wav,
    Ogg,
    Flac,
}

impl AudioFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Wav => "wav",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Flac => "flac",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            AudioFormat::Mp3 => "audio/mpeg",
            AudioFormat::Wav => "audio/wav",
            AudioFormat::Ogg => "audio/ogg",
            AudioFormat::Flac => "audio/flac",
        }
    }
}

/// Output from TTS generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutput {
    pub path: PathBuf,
    pub format: AudioFormat,
    pub duration: Duration,
    pub segments: Vec<AudioSegment>,
    pub voice: Voice,
}

/// Individual audio segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSegment {
    pub path: PathBuf,
    pub text: String,
    pub duration: Duration,
    pub voice: Voice,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn voice_selector_defaults() {
        let selector = VoiceSelector::new();
        assert_eq!(selector.language, "en-US");
        assert_eq!(selector.gender, Some(VoiceGender::Neutral));
        assert_eq!(selector.style, Some(VoiceStyle::News));
    }

    #[test]
    fn voice_selector_builder() {
        let selector = VoiceSelector::new()
            .with_voice("nova".to_string())
            .with_language("en-GB".to_string())
            .with_gender(VoiceGender::Female)
            .with_style(VoiceStyle::Conversational);

        assert_eq!(selector.preferred_voice, Some("nova".to_string()));
        assert_eq!(selector.language, "en-GB");
        assert_eq!(selector.gender, Some(VoiceGender::Female));
        assert_eq!(selector.style, Some(VoiceStyle::Conversational));
    }

    #[test]
    fn voice_select_returns_voice() {
        let selector = VoiceSelector::new();
        let voice = selector.select("test text").unwrap();
        assert_eq!(voice.name, "default");
        assert_eq!(voice.language, "en-US");
    }

    #[test]
    fn audio_format_extensions() {
        assert_eq!(AudioFormat::Mp3.extension(), "mp3");
        assert_eq!(AudioFormat::Wav.extension(), "wav");
        assert_eq!(AudioFormat::Ogg.extension(), "ogg");
        assert_eq!(AudioFormat::Flac.extension(), "flac");
    }

    #[test]
    fn tts_generator_defaults() {
        let gen = TTSGenerator::new();
        assert_eq!(gen.rate, 1.0);
        assert_eq!(gen.format, AudioFormat::Mp3);
        assert_eq!(gen.max_chunk_chars, 4000);
    }

    #[test]
    fn tts_chunk_text() {
        let gen = TTSGenerator::new().with_format(AudioFormat::Mp3);
        let short = "Short text";
        assert_eq!(gen.chunk_text(short).len(), 1);

        let long = "A".repeat(5000);
        let chunks = gen.chunk_text(&long);
        assert!(chunks.len() >= 2);
    }

    #[tokio::test]
    async fn tts_generate_creates_output() {
        let gen = TTSGenerator::new()
            .with_output_dir(std::path::PathBuf::from("/tmp/test_tts"))
            .with_format(AudioFormat::Mp3);

        let date = Utc.with_ymd_and_hms(2026, 1, 15, 6, 0, 0).unwrap();
        let output = gen.generate("Test report content", &date).await.unwrap();

        assert!(output.path.exists());
        assert_eq!(output.format, AudioFormat::Mp3);
        assert!(output.duration.as_secs() > 0);

        // Cleanup
        let _ = tokio::fs::remove_dir_all("/tmp/test_tts").await;
    }
}