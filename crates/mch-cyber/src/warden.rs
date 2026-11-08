//! Warden Service — Prompt injection and backdoor detection
//!
//! Forged from Spotlight (CMUL8/spotlight) WardenService
//! Implements multi-layer detection for adversarial inputs targeting the scope authority

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Warden configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WardenConfig {
    /// Enable heuristic pattern matching
    pub enable_heuristics: bool,
    /// Enable semantic analysis (placeholder for ML-based detection)
    pub enable_semantic: bool,
    /// Minimum confidence threshold for alerts (0.0-1.0)
    pub alert_threshold: f64,
    /// Maximum payload size to analyze (bytes)
    pub max_payload_size: usize,
    /// Custom denial patterns
    pub custom_patterns: Vec<String>,
}

impl Default for WardenConfig {
    fn default() -> Self {
        Self {
            enable_heuristics: true,
            enable_semantic: false,
            alert_threshold: 0.7,
            max_payload_size: 1024 * 1024, // 1 MB
            custom_patterns: Vec::new(),
        }
    }
}

/// Warden configuration builder
pub struct WardenConfigBuilder {
    config: WardenConfig,
}

impl WardenConfigBuilder {
    pub fn new() -> Self {
        Self { config: WardenConfig::default() }
    }

    pub fn enable_heuristics(mut self, enable: bool) -> Self {
        self.config.enable_heuristics = enable;
        self
    }

    pub fn enable_semantic(mut self, enable: bool) -> Self {
        self.config.enable_semantic = enable;
        self
    }

    pub fn alert_threshold(mut self, threshold: f64) -> Self {
        self.config.alert_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn max_payload_size(mut self, size: usize) -> Self {
        self.config.max_payload_size = size;
        self
    }

    pub fn add_custom_pattern(mut self, pattern: String) -> Self {
        self.config.custom_patterns.push(pattern);
        self
    }

    pub fn build(self) -> WardenConfig {
        self.config
    }
}

impl Default for WardenConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Risk level for actions/requests (OmniPilot-style granular permissions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Classification of detected threats
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreatClassification {
    /// Direct prompt injection attempt
    PromptInjection,
    /// Encoded/obfuscated injection
    ObfuscatedInjection,
    /// Backdoor trigger pattern
    BackdoorTrigger,
    /// Scope manipulation attempt
    ScopeManipulation,
    /// Data exfiltration attempt
    DataExfiltration,
    /// Tool misuse / command injection
    CommandInjection,
    /// Benign - no threat detected
    Benign,
}

/// Result of a Warden scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    /// Overall threat classification
    pub classification: ThreatClassification,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Matched patterns with locations
    pub matches: Vec<PatternMatch>,
    /// Whether the payload should be blocked
    pub should_block: bool,
    /// Human-readable explanation
    pub explanation: String,
}

/// A matched pattern with location info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    /// The pattern that matched
    pub pattern: String,
    /// Classification of this match
    pub classification: ThreatClassification,
    /// Byte offset in payload
    pub offset: usize,
    /// Length of match
    pub length: usize,
    /// Confidence of this specific match
    pub confidence: f64,
}

/// A matched backdoor pattern in a diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackdoorMatch {
    /// The pattern that matched
    pub pattern: String,
    /// Classification
    pub classification: ThreatClassification,
    /// Line number in diff
    pub line: usize,
    /// Context around the match
    pub context: String,
    /// Confidence of this match
    pub confidence: f64,
}

/// Injection match in target content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionMatch {
    /// The pattern that matched
    pub pattern: String,
    /// Classification of this match
    pub classification: ThreatClassification,
    /// Byte offset in payload
    pub offset: usize,
    /// Length of match
    pub length: usize,
    /// Confidence of this match
    pub confidence: f64,
}

/// Warden errors
#[derive(Debug, Error)]
pub enum WardenError {
    #[error("payload too large: {size} > {max}")]
    PayloadTooLarge { size: usize, max: usize },
    #[error("scanner error: {0}")]
    ScannerError(String),
}

/// Envelope sentinels for wrapping untrusted content
const ENV_BEGIN_PREFIX: &str = "===UNTRUSTED_CONTENT_BEGIN";
const ENV_END: &str = "===UNTRUSTED_CONTENT_END===";

/// Warden Service — analyzes payloads for adversarial content
pub struct WardenService {
    config: WardenConfig,
    /// Compiled heuristic patterns
    patterns: Vec<HeuristicPattern>,
}

#[derive(Debug, Clone)]
struct HeuristicPattern {
    pattern: String,
    classification: ThreatClassification,
    regex: regex::Regex,
    weight: f64,
}

impl WardenService {
    /// Create a new Warden detector with default configuration
    pub fn new(config: WardenConfig) -> Result<Self, WardenError> {
        let patterns = Self::compile_patterns(&config)?;
        Ok(Self { config, patterns })
    }

    /// Create with default configuration
    pub fn default() -> Result<Self, WardenError> {
        Self::new(WardenConfig::default())
    }

    /// Scan a payload for threats
    pub fn scan(&self, payload: &str, origin: &str) -> Result<DetectionResult, WardenError> {
        if payload.len() > self.config.max_payload_size {
            return Err(WardenError::PayloadTooLarge {
                size: payload.len(),
                max: self.config.max_payload_size,
            });
        }

        let mut matches = Vec::new();

        // Heuristic pattern matching
        if self.config.enable_heuristics {
            for hp in &self.patterns {
                for mat in hp.regex.find_iter(payload) {
                    matches.push(PatternMatch {
                        pattern: hp.pattern.clone(),
                        classification: hp.classification,
                        offset: mat.start(),
                        length: mat.len(),
                        confidence: hp.weight,
                    });
                }
            }
        }

        // Custom patterns
        for custom in &self.config.custom_patterns {
            if let Ok(re) = regex::Regex::new(custom) {
                for mat in re.find_iter(payload) {
                    matches.push(PatternMatch {
                        pattern: custom.clone(),
                        classification: ThreatClassification::PromptInjection,
                        offset: mat.start(),
                        length: mat.len(),
                        confidence: 0.8,
                    });
                }
            }
        }

        // Analyze matches and determine overall classification
        let (classification, confidence, should_block, explanation) = self.analyze_matches(&matches, payload);

        Ok(DetectionResult {
            classification,
            confidence,
            matches,
            should_block,
            explanation,
        })
    }

    /// Scan and return error if threat detected (for interceptor integration)
    pub fn scan_or_block(&self, payload: &str, origin: &str) -> Result<(), WardenError> {
        let result = self.scan(payload, origin)?;
        if result.should_block {
            Err(WardenError::ScannerError(result.explanation))
        } else {
            Ok(())
        }
    }

    /// Wrap target content in a hardened envelope
    pub fn wrap_target_content(&self, text: &str, origin: &str) -> String {
        if text.trim().starts_with(ENV_BEGIN_PREFIX) && text.contains(ENV_END) {
            return text.to_string();
        }
        let safe_origin = origin.replace('\n', " ").trim().to_string();
        if safe_origin.is_empty() {
            return format!(
                "\n{} [origin={}]===\n{}\n{}\n",
                ENV_BEGIN_PREFIX, safe_origin, text, ENV_END
            );
        }
        format!(
            "\n{} [origin={}]===\n{}\n{}\n",
            ENV_BEGIN_PREFIX, safe_origin, text, ENV_END
        )
    }

    /// Check if text is already wrapped
    pub fn is_wrapped(&self, text: &str) -> bool {
        text.trim().starts_with(ENV_BEGIN_PREFIX) && text.contains(ENV_END)
    }

    fn compile_patterns(config: &WardenConfig) -> Result<Vec<HeuristicPattern>, WardenError> {
        let mut patterns = Vec::new();

        // Prompt injection patterns
        let injection_patterns = [
            ("ignore previous instructions", ThreatClassification::PromptInjection, 0.9),
            ("disregard.*(?:above|prior|previous|system)", ThreatClassification::PromptInjection, 0.85),
            ("you are now", ThreatClassification::PromptInjection, 0.7),
            ("act as", ThreatClassification::PromptInjection, 0.65),
            ("pretend to be", ThreatClassification::PromptInjection, 0.65),
            ("system prompt", ThreatClassification::PromptInjection, 0.8),
            ("reveal.*prompt", ThreatClassification::PromptInjection, 0.75),
            ("output.*prompt", ThreatClassification::PromptInjection, 0.75),
            ("show.*instructions", ThreatClassification::PromptInjection, 0.75),
            ("bypass.*(?:filter|guard|safety)", ThreatClassification::PromptInjection, 0.85),
            ("jailbreak", ThreatClassification::PromptInjection, 0.9),
            ("DAN", ThreatClassification::PromptInjection, 0.8),
            ("developer mode", ThreatClassification::PromptInjection, 0.75),
        ];

        for (pattern, classification, weight) in injection_patterns {
            let regex = regex::Regex::new(&format!(r"(?i){pattern}"))
                .map_err(|e| WardenError::ScannerError(format!("Invalid regex: {e}")))?;
            patterns.push(HeuristicPattern {
                pattern: pattern.into(),
                classification,
                regex,
                weight,
            });
        }

        // Obfuscation patterns
        let obfuscation_patterns = [
            (r"\\x[0-9a-fA-F]{2}", ThreatClassification::ObfuscatedInjection, 0.7),
            (r"\\u[0-9a-fA-F]{4}", ThreatClassification::ObfuscatedInjection, 0.7),
            (r"%[0-9a-fA-F]{2}", ThreatClassification::ObfuscatedInjection, 0.6),
            (r"base64[,:]\s*[A-Za-z0-9+/=]+", ThreatClassification::ObfuscatedInjection, 0.75),
            (r"rot13", ThreatClassification::ObfuscatedInjection, 0.6),
            (r"(?i)eval\s*\(", ThreatClassification::ObfuscatedInjection, 0.8),
            (r"(?i)exec\s*\(", ThreatClassification::ObfuscatedInjection, 0.8),
        ];

        for (pattern, classification, weight) in obfuscation_patterns {
            let regex = regex::Regex::new(pattern)
                .map_err(|e| WardenError::ScannerError(format!("Invalid regex: {e}")))?;
            patterns.push(HeuristicPattern {
                pattern: pattern.into(),
                classification,
                regex,
                weight,
            });
        }

        // Backdoor trigger patterns
        let backdoor_patterns = [
            ("trigger_word", ThreatClassification::BackdoorTrigger, 0.85),
            ("magic_token", ThreatClassification::BackdoorTrigger, 0.85),
            ("backdoor", ThreatClassification::BackdoorTrigger, 0.8),
            ("secret_key", ThreatClassification::BackdoorTrigger, 0.75),
            ("admin_override", ThreatClassification::BackdoorTrigger, 0.8),
        ];

        for (pattern, classification, weight) in backdoor_patterns {
            let regex = regex::Regex::new(&format!(r"(?i)\b{pattern}\b"))
                .map_err(|e| WardenError::ScannerError(format!("Invalid regex: {e}")))?;
            patterns.push(HeuristicPattern {
                pattern: pattern.into(),
                classification,
                regex,
                weight,
            });
        }

        // Scope manipulation patterns
        let scope_patterns = [
            (r"(?i)scope\s*[:=]\s*\[\s*\]", ThreatClassification::ScopeManipulation, 0.9),
            (r"(?i)in_scope\s*[:=]\s*\[\s*\*\s*\]", ThreatClassification::ScopeManipulation, 0.85),
            (r"(?i)out_of_scope\s*[:=]\s*\[\s*\]", ThreatClassification::ScopeManipulation, 0.7),
            ("allow all", ThreatClassification::ScopeManipulation, 0.8),
            ("disable scope", ThreatClassification::ScopeManipulation, 0.85),
        ];

        for (pattern, classification, weight) in scope_patterns {
            let regex = regex::Regex::new(&format!(r"(?i){pattern}"))
                .map_err(|e| WardenError::ScannerError(format!("Invalid regex: {e}")))?;
            patterns.push(HeuristicPattern {
                pattern: pattern.into(),
                classification,
                regex,
                weight,
            });
        }

        // Command injection patterns
        let cmd_patterns = [
            (r";\s*(?:rm|cat|ls|curl|wget|nc|bash|sh)\b", ThreatClassification::CommandInjection, 0.9),
            (r"\|\s*(?:sh|bash)\b", ThreatClassification::CommandInjection, 0.85),
            (r"`[^`]*`", ThreatClassification::CommandInjection, 0.7),
            (r"\$\([^)]*\)", ThreatClassification::CommandInjection, 0.7),
            (r">\s*/dev/", ThreatClassification::CommandInjection, 0.75),
        ];

        for (pattern, classification, weight) in cmd_patterns {
            let regex = regex::Regex::new(pattern)
                .map_err(|e| WardenError::ScannerError(format!("Invalid regex: {e}")))?;
            patterns.push(HeuristicPattern {
                pattern: pattern.into(),
                classification,
                regex,
                weight,
            });
        }

        Ok(patterns)
    }

    fn analyze_matches(
        &self,
        matches: &[PatternMatch],
        payload: &str,
    ) -> (ThreatClassification, f64, bool, String) {
        if matches.is_empty() {
            return (
                ThreatClassification::Benign,
                0.0,
                false,
                "No threats detected".into(),
            );
        }

        // Group by classification
        use std::collections::BTreeMap;
        let mut by_class: BTreeMap<ThreatClassification, Vec<&PatternMatch>> = BTreeMap::new();
        for m in matches {
            by_class.entry(m.classification).or_default().push(m);
        }

        // Find highest confidence classification
        let mut best_class = ThreatClassification::Benign;
        let mut best_confidence = 0.0;
        let mut explanations = Vec::new();

        for (class, class_matches) in &by_class {
            let max_conf = class_matches.iter().map(|m| m.confidence).fold(0.0, f64::max);
            let count = class_matches.len() as f64;
            // Boost confidence for multiple matches of same class
            let combined_conf = (max_conf * (1.0 + 0.1 * count.ln().max(0.0))).min(1.0);

            if combined_conf > best_confidence {
                best_confidence = combined_conf;
                best_class = *class;
            }

            for m in class_matches {
                explanations.push(format!(
                    "{} at offset {} (conf: {:.2})",
                    m.pattern, m.offset, m.confidence
                ));
            }
        }

        let should_block = best_confidence >= self.config.alert_threshold
            && best_class != ThreatClassification::Benign;

        let explanation = if should_block {
            format!(
                "Threat detected: {:?} (confidence: {:.2}). Matches: {}",
                best_class,
                best_confidence,
                explanations.join("; ")
            )
        } else {
            format!(
                "Low-confidence signals: {:?} (confidence: {:.2}). Matches: {}",
                best_class,
                best_confidence,
                explanations.join("; ")
            )
        };

        (best_class, best_confidence, should_block, explanation)
    }
}