//! Single Character Ingest Library
//!
//! Library module for single character ingestion functionality.
//! CLI has been moved to src/bin/single_character_ingest.rs

use std::path::{Path, PathBuf};

// Re-export commonly used types for the API
pub use crate::ingest::chunking_utils::chunk_entity_fields;
pub use crate::ingest::ingest_utils;

/// Configuration for single character processing
#[derive(Debug, Clone)]
pub struct SingleCharacterConfig {
    /// Maximum embedding length for chunks
    pub max_embed_len: usize,
    /// Maximum retry attempts for embedding
    pub max_retries: usize,
    /// Retry delay in seconds
    pub retry_delay: usize,
    /// Model name for embeddings
    pub model: String,
}

impl Default for SingleCharacterConfig {
    fn default() -> Self {
        Self {
            max_embed_len: 250,
            max_retries: 3,
            retry_delay: 5,
            model: "harald-phi4".to_string(),
        }
    }
}

/// Result of single character processing
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// Number of files created
    pub files_created: usize,
    /// Number of embeddings generated
    pub embeddings_generated: usize,
    /// Success status
    pub success: bool,
    /// Error message if any
    pub error: Option<String>,
}

/// Process a single character entry (placeholder implementation)
///
/// This function will be implemented in a future refactoring to extract
/// the complex processing logic from the original main function.
///
/// # Arguments
/// * `character_data` - The JSON character data to process
/// * `output_dir` - Directory to write output files
/// * `config` - Processing configuration
///
/// # Returns
/// Returns a `ProcessingResult` with processing statistics and status.
pub fn process_character(
    _character_data: &serde_json::Value,
    _output_dir: &Path,
    _config: &SingleCharacterConfig,
) -> Result<ProcessingResult, Box<dyn std::error::Error>> {
    // TODO: Implement the actual processing logic by extracting it from the original main function
    // This is a placeholder to allow the build to succeed during migration
    Err("Single character processing is not yet implemented - under refactoring".into())
}

/// Validate a single character JSON entry
///
/// Checks that the character entry has required fields and valid structure.
pub fn validate_character_entry(character: &serde_json::Value) -> Result<(), String> {
    if !character.is_object() {
        return Err("Character entry must be a JSON object".to_string());
    }

    let obj = character.as_object().unwrap();

    if !obj.contains_key("character_name") {
        return Err("Character entry must have 'character_name' field".to_string());
    }

    if let Some(name) = obj.get("character_name").and_then(|n| n.as_str()) {
        if name.trim().is_empty() {
            return Err("Character name cannot be empty".to_string());
        }
    } else {
        return Err("Character name must be a string".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_single_character_config_default() {
        let config = SingleCharacterConfig::default();
        assert_eq!(config.max_embed_len, 250);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay, 5);
        assert_eq!(config.model, "harald-phi4");
    }

    #[test]
    fn test_validate_character_entry_valid() {
        let character = json!({
            "character_name": "Vision",
            "description": "A test character"
        });

        let result = validate_character_entry(&character);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_character_entry_missing_name() {
        let character = json!({
            "description": "A test character"
        });

        let result = validate_character_entry(&character);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("character_name"));
    }

    #[test]
    fn test_validate_character_entry_empty_name() {
        let character = json!({
            "character_name": "",
            "description": "A test character"
        });

        let result = validate_character_entry(&character);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_validate_character_entry_invalid_structure() {
        let character = json!("not an object");

        let result = validate_character_entry(&character);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("JSON object"));
    }

    #[test]
    fn test_process_character_placeholder() {
        let character = json!({
            "character_name": "Vision"
        });
        let output_dir = PathBuf::from("/tmp");
        let config = SingleCharacterConfig::default();

        let result = process_character(&character, &output_dir, &config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not yet implemented"));
    }
}
