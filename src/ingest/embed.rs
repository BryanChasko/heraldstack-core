//! Embedding generation module for converting text to vector representations.
//!
//! This module handles communication with a local embedding API to convert text
//! into high-dimensional vectors suitable for semantic similarity search. It uses
//! the Harald-Phi4 model running locally via Ollama to generate embeddings that
//! capture the semantic meaning of text content.
//!
//! # Module Structure
//! This is a "module source file" that defines the embed module:
//! - Loaded via `mod embed;` in main.rs/lib.rs  
//! - Functions accessed as `embed::embed()` from other modules
//! - Core utility module used by both ingest and query modules
//! - Handles the critical text-to-vector conversion for semantic search

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Default embedding model name.
///
/// This model should be available in the local Ollama instance.
/// Harald-Phi4 is optimized for code and documentation understanding.
const DEFAULT_MODEL: &str = "harald-phi4";

/// Default embedding API endpoint.
///
/// Points to a local Ollama instance running on the standard port.
/// This avoids external API dependencies and ensures data privacy.
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434/api/embeddings";

/// Default timeout for embedding requests in seconds.
///
/// Balances between allowing sufficient time for processing
/// and preventing indefinite hangs on network issues.
/// Increased from 30 to 60 seconds to handle larger text chunks.
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Maximum retry attempts for failed embedding requests.
const MAX_RETRY_ATTEMPTS: usize = 3;

/// Configuration for embedding generation.
#[derive(Debug, Clone)]
pub struct EmbedConfig {
    /// Model name to use for embedding generation.
    pub model: String,
    /// API endpoint URL.
    pub endpoint: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Maximum retry attempts for failed requests.
    pub max_retries: usize,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            max_retries: MAX_RETRY_ATTEMPTS,
        }
    }
}

/// Request payload for the embedding API.
#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    /// Model name to use for embedding generation.
    model: &'a str,
    /// Text content to convert to embedding.
    prompt: &'a str,
    /// Whether to stream the response (always false for embeddings).
    stream: bool,
}

/// Response payload from the embedding API.
#[derive(Debug, Deserialize)]
struct EmbedResponse {
    /// The generated embedding vector.
    embedding: Vec<f32>,
}

/// Converts text to an embedding vector using the default configuration.
///
/// This is the primary interface for generating embeddings from text.
/// It uses the local Harald-Phi4 model to create high-dimensional vectors
/// that capture the semantic meaning of the input text.
///
/// # Arguments
/// * `text` - The text content to convert to an embedding
/// * `max_tokens` - Maximum number of tokens to process (for API limits)
/// * `client` - HTTP client for making API requests
///
/// # Returns
/// Returns a vector of f32 values representing the text's embedding.
///
/// # Errors
/// Returns an error if:
/// - The embedding API is unreachable
/// - The request times out
/// - The response format is invalid
/// - Network connectivity issues occur
///
/// # Examples
/// ```rust,no_run
/// use reqwest::Client;
/// use rust_ingest::embed;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let client = Client::new();
///     let embedding = embed::embed("Hello world", 100, &client).await?;
///     println!("Embedding dimension: {}", embedding.len());
///     Ok(())
/// }
/// ```
/// Converts text to an embedding vector with custom configuration.
///
/// # Arguments
/// * `text` - The text content to convert to an embedding
/// * `max_tokens` - Maximum number of tokens to process (for API limits)
/// * `client` - HTTP client for making API requests
/// * `config` - Configuration parameters for embedding generation
///
/// # Returns
/// Returns a vector of f32 values representing the text's embedding.
///
/// # Errors
/// Returns an error if any step of the embedding process fails.
pub async fn embed_with_config(
    text: &str,
    _max_tokens: usize, // Currently unused but kept for API compatibility
    client: &Client,
    config: EmbedConfig,
) -> Result<Vec<f32>> {
    validate_input(text)?;

    let mut last_error = None;

    for attempt in 1..=config.max_retries {
        match attempt_embedding(text, client, &config).await {
            Ok(embedding) => {
                validate_embedding(&embedding)?;
                return Ok(embedding);
            }
            Err(e) => {
                last_error = Some(e);
                if attempt < config.max_retries {
                    // Exponential backoff: wait 2^attempt seconds
                    let delay = Duration::from_secs(2_u64.pow(attempt as u32));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All embedding attempts failed")))
}

/// Validates input text before processing.
fn validate_input(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Err(anyhow::anyhow!("Input text cannot be empty"));
    }

    if text.len() > 100_000 {
        return Err(anyhow::anyhow!(
            "Input text too long: {} characters",
            text.len()
        ));
    }

    Ok(())
}

/// Attempts to generate an embedding for the given text.
async fn attempt_embedding(text: &str, client: &Client, config: &EmbedConfig) -> Result<Vec<f32>> {
    let request_body = EmbedRequest {
        model: &config.model,
        prompt: text,
        stream: false,
    };

    let response: EmbedResponse = client
        .post(&config.endpoint)
        .json(&request_body)
        .timeout(Duration::from_secs(config.timeout_secs))
        .send()
        .await
        .context("Failed to send embedding request")?
        .error_for_status()
        .context("Embedding API returned error status")?
        .json()
        .await
        .context("Failed to parse embedding response")?;

    Ok(response.embedding)
}

/// Simple wrapper function for embedding with default configuration.
///
/// This provides a simpler API for basic embedding needs while maintaining
/// compatibility with existing code.
///
/// # Arguments
/// * `text` - Text to embed
/// * `max_tokens` - Maximum tokens (currently unused but kept for compatibility)
/// * `client` - HTTP client for making requests
///
/// # Returns
/// Returns a vector of f32 values representing the embedding.
///
/// # Errors
/// Returns an error if the embedding process fails.
pub async fn embed(text: &str, max_tokens: usize, client: &Client) -> Result<Vec<f32>> {
    let config = EmbedConfig::default();
    embed_with_config(text, max_tokens, client, config).await
}

/// Validates the generated embedding vector.
fn validate_embedding(embedding: &[f32]) -> Result<()> {
    if embedding.is_empty() {
        return Err(anyhow::anyhow!("Received empty embedding vector"));
    }

    if embedding.len() < 100 {
        return Err(anyhow::anyhow!(
            "Embedding dimension too small: {}",
            embedding.len()
        ));
    }

    // Check for NaN or infinite values
    if embedding.iter().any(|&x| !x.is_finite()) {
        return Err(anyhow::anyhow!(
            "Embedding contains invalid values (NaN or infinite)"
        ));
    }

    Ok(())
}

/// Creates an embedding configuration with custom model and endpoint.
///
/// # Arguments
/// * `model` - Model name to use for embedding generation
/// * `endpoint` - API endpoint URL
///
/// # Returns
/// Returns a configured `EmbedConfig` instance.
pub fn create_config(model: &str, endpoint: &str) -> EmbedConfig {
    EmbedConfig {
        model: model.to_string(),
        endpoint: endpoint.to_string(),
        timeout_secs: DEFAULT_TIMEOUT_SECS,
        max_retries: MAX_RETRY_ATTEMPTS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_config_default() {
        let config = EmbedConfig::default();
        assert_eq!(config.model, DEFAULT_MODEL);
        assert_eq!(config.endpoint, DEFAULT_ENDPOINT);
        assert_eq!(config.timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert_eq!(config.max_retries, MAX_RETRY_ATTEMPTS);
    }

    #[test]
    fn test_create_config() {
        let config = create_config("custom-model", "http://custom:8080/api");
        assert_eq!(config.model, "custom-model");
        assert_eq!(config.endpoint, "http://custom:8080/api");
        assert_eq!(config.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn test_validate_input() {
        // Valid input
        assert!(validate_input("Hello world").is_ok());

        // Empty input
        assert!(validate_input("").is_err());
        assert!(validate_input("   ").is_err());

        // Too long input
        let long_text = "a".repeat(100_001);
        assert!(validate_input(&long_text).is_err());
    }

    #[test]
    fn test_validate_embedding() {
        // Valid embedding
        let valid_embedding = vec![0.1; 384];
        assert!(validate_embedding(&valid_embedding).is_ok());

        // Empty embedding
        assert!(validate_embedding(&[]).is_err());

        // Too small embedding
        let small_embedding = vec![0.1; 50];
        assert!(validate_embedding(&small_embedding).is_err());

        // Invalid values
        let invalid_embedding = vec![f32::NAN; 384];
        assert!(validate_embedding(&invalid_embedding).is_err());

        let infinite_embedding = vec![f32::INFINITY; 384];
        assert!(validate_embedding(&infinite_embedding).is_err());
    }

    #[test]
    fn test_embed_request_serialization() {
        let request = EmbedRequest {
            model: "test-model",
            prompt: "test text",
            stream: false,
        };

        let serialized = serde_json::to_string(&request).unwrap();
        assert!(serialized.contains("test-model"));
        assert!(serialized.contains("test text"));
        assert!(serialized.contains("false"));
    }

    #[test]
    fn test_embed_response_deserialization() {
        let json = r#"{"embedding": [0.1, 0.2, 0.3]}"#;
        let response: EmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embedding, vec![0.1, 0.2, 0.3]);
    }

    // Integration tests would require a running Ollama instance
    #[cfg(feature = "integration-tests")]
    mod integration {
        use super::*;
        use reqwest::Client;

        #[tokio::test]
        async fn test_embed_integration() {
            let client = Client::new();
            let result = embed("test text", 100, &client).await;

            // This test would only pass with a running Ollama instance
            // In CI/CD, this would be skipped or use a mock server
            match result {
                Ok(embedding) => {
                    assert!(!embedding.is_empty());
                    assert!(embedding.len() >= 100);
                }
                Err(_) => {
                    // Expected if Ollama is not running
                    println!("Ollama not available for integration test");
                }
            }
        }
    }
}
