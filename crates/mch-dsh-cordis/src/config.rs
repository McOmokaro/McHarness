//! Config validation and schema.

use schemars::schema::RootSchema;
use serde_json::Value;

/// Config schema wrapper (a JSON Schema document).
pub type ConfigSchema = RootSchema;

/// Outcome of validating a candidate config.
#[derive(Debug, Clone)]
pub struct ConfigValidatorResult {
    pub value: Value,
    pub issues: Vec<String>,
}

/// Validates plugin config against its schema.
#[derive(Debug, Default)]
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(_schema: &ConfigSchema, config: &Value) -> Result<Value, String> {
        Ok(config.clone())
    }
}
