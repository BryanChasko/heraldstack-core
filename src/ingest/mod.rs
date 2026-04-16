//! Ingest module.
//!
//! This module provides functionality for ingesting data into the system.

pub mod chunked_ingest;
pub mod chunking_utils;
pub mod embed;
pub mod ingest;
pub mod ingest_utils;
pub mod query;
pub mod single_character_ingest;

// Re-export commonly used items
pub use embed::{embed, embed_with_config, EmbedConfig};
pub use ingest::{run_with_config, IngestConfig};
pub use query::QueryConfig;
