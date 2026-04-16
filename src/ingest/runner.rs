//! File ingestion module for semantic search indexing.
//!
//! This module handles the ingestion of files into a searchable vector index.
//! It processes files, generates embeddings, and builds an HNSW index for semantic search
//! using the HNSW algorithm for efficient nearest neighbor search in high-dimensional spaces.
//! This creates a searchable database of file contents based on their semantic meaning.
//!
//! # Module Structure
//! In Rust, this .rs file defines a module named "ingest":
//! - If main.rs/lib.rs contains `mod ingest;`, Rust loads this file as the ingest module
//! - Functions here are accessed as `ingest::run()` from other modules
//! - This is a "module source file" - a unit of compilation within our crate
//! - Part of the flat module style (modern) vs ingest/mod.rs (legacy)

use anyhow::{Context, Result};
use hnsw_rs::prelude::*;
use serde_json::json;
use std::{fs::File, path::PathBuf};
use walkdir::WalkDir;

use crate::ingest::embed;

/// Directories to skip during file traversal.
///
/// These directories typically contain:
/// - Version control metadata (.git)
/// - Virtual environments (.venv)
/// - Build artifacts (target, node_modules)
/// - Development files (.vscode, .cargo)
/// - Other non-relevant content
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".venv",
    ".cargo",
    ".github",
    ".vscode",
    "target",
    "node_modules",
    "build",
    "dist",
    "docs/api",
    "src/target",
    "src/Cargo.lock",
];

/// Maximum number of characters to read from each file for embedding.
///
/// This limit serves multiple purposes:
/// - Controls API costs for embedding services
/// - Prevents memory issues with extremely large files
/// - Ensures consistent processing time per file
const MAX_FILE_CHARS: usize = 800;

/// Maximum number of tokens for embedding API requests.
const MAX_EMBEDDING_TOKENS: usize = 600;

/// HNSW index construction parameters optimized for semantic search.
///
/// - `MAX_CONNECTIONS`: Maximum connections per node, controls index quality and memory usage
/// - `EF_CONSTRUCTION`: Size of dynamic candidate list during construction, higher = better quality but slower build
/// - `MAX_LAYER`: Maximum layer in the hierarchical structure, influences search performance
/// - `MAX_ELEMENTS`: Maximum number of elements that can be stored in the index
const HNSW_MAX_CONNECTIONS: usize = 16;
const HNSW_MAX_ELEMENTS: usize = 100_000;
const HNSW_EF_CONSTRUCTION: usize = 200;
const HNSW_MAX_LAYER: usize = 16;

/// Progress reporting interval (number of files).
const PROGRESS_INTERVAL: usize = 10;

/// Supported file extensions for semantic indexing.
const SUPPORTED_EXTENSIONS: &[&str] = &["md", "json", "jsonl"];

/// Configuration for the ingestion process.
#[derive(Debug, Clone)]
pub struct IngestConfig {
    /// Root directory to start ingestion from.
    pub root_dir: PathBuf,
    /// Maximum characters to read per file.
    pub max_chars: usize,
    /// Maximum tokens for embedding requests.
    pub max_tokens: usize,
    /// Maximum number of files to process concurrently.
    /// If None, defaults to the number of CPU cores.
    pub max_concurrent_files: Option<usize>,
}

impl Default for IngestConfig {
    fn default() -> Self {
        Self {
            root_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            max_chars: MAX_FILE_CHARS,
            max_tokens: MAX_EMBEDDING_TOKENS,
            max_concurrent_files: None, // Use CPU core count by default
        }
    }
}

/// Statistics from the ingestion process.
#[derive(Debug, Clone)]
pub struct IngestStats {
    /// Total number of files processed.
    pub files_processed: usize,
    /// Total number of files skipped.
    pub files_skipped: usize,
    /// Output directory path.
    pub output_dir: PathBuf,
}

/// Main ingestion function that processes files and builds a searchable vector index.
///
/// This function creates the foundation for semantic search across the codebase by:
/// 1. Traversing the file system to find relevant files
/// 2. Reading and preprocessing file contents
/// 3. Generating embeddings for semantic representation
/// 4. Building an HNSW index for efficient similarity search
/// 5. Persisting the index and metadata for later use
///
/// # Returns
/// Returns `IngestStats` containing information about the ingestion process.
///
/// # Errors
/// Returns an error if:
/// - File system operations fail
/// - Embedding API requests fail
/// - Index serialization fails
/// - Directory creation fails
pub async fn run() -> Result<IngestStats> {
    run_with_config(IngestConfig::default()).await
}

/// Runs ingestion with custom configuration.
///
/// # Arguments
/// * `config` - Configuration parameters for the ingestion process
///
/// # Returns
/// Returns `IngestStats` containing information about the ingestion process.
///
/// # Errors
/// Returns an error if any step of the ingestion process fails.
pub async fn run_with_config(config: IngestConfig) -> Result<IngestStats> {
    let client = create_http_client()?;
    let index = create_hnsw_index();
    let mut file_metadata = Vec::new();
    let mut stats = IngestStats {
        files_processed: 0,
        files_skipped: 0,
        output_dir: config.root_dir.join("data"),
    };

    // Process all files in the directory tree
    process_directory_tree(&config, &client, &index, &mut file_metadata, &mut stats).await?;

    // Persist the index and metadata
    persist_index_data(&index, &file_metadata, &stats.output_dir)?;

    println!(
        "Ingestion complete: {} files processed, {} files skipped → {}",
        stats.files_processed,
        stats.files_skipped,
        stats.output_dir.join("index.hnsw.*").display()
    );

    Ok(stats)
}

/// Creates an HTTP client for embedding API requests.
fn create_http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90)) // Increased to accommodate embedding timeouts
        .build()
        .context("Failed to create HTTP client")
}

/// Creates and configures an HNSW index for vector similarity search.
///
/// # Returns
/// A new HNSW index configured with optimal parameters for semantic search.
fn create_hnsw_index() -> Hnsw<'static, f32, DistCosine> {
    Hnsw::<'static, f32, DistCosine>::new(
        HNSW_MAX_CONNECTIONS,
        HNSW_MAX_ELEMENTS,
        HNSW_MAX_LAYER,
        HNSW_EF_CONSTRUCTION,
        DistCosine {},
    )
}

/// Processes all files in the directory tree with parallel execution.
async fn process_directory_tree(
    config: &IngestConfig,
    client: &reqwest::Client,
    index: &Hnsw<'_, f32, DistCosine>,
    file_metadata: &mut Vec<PathBuf>,
    stats: &mut IngestStats,
) -> Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    // Create a semaphore to limit concurrent operations
    let max_concurrent = config.max_concurrent_files.unwrap_or_else(|| {
        // Default to number of CPUs if not specified
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    });

    // Use a semaphore to limit concurrent embedding operations
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    // Use JoinSet to manage async tasks
    let mut tasks = JoinSet::new();

    // Use atomics for thread-safe counters
    let processed_count = Arc::new(AtomicUsize::new(0));
    let skipped_count = Arc::new(AtomicUsize::new(0));

    // Use a mutex to protect the file metadata vector
    let file_paths = Arc::new(Mutex::new(Vec::new()));

    // Collect candidate files first
    let mut candidate_files = Vec::new();

    // First pass: find all valid files to process
    for entry in WalkDir::new(&config.root_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if should_skip_path(path) {
            stats.files_skipped += 1;
            continue;
        }

        if !is_supported_file(path) {
            stats.files_skipped += 1;
            continue;
        }

        // Add to candidates
        candidate_files.push(path.to_path_buf());
    }

    println!("Found {} files to process", candidate_files.len());

    // Second pass: process files concurrently
    for (file_id, path) in candidate_files.into_iter().enumerate() {
        // Clone references for the async task
        let semaphore_clone = semaphore.clone();
        let client_clone = client.clone();
        let config_clone = config.clone();
        let processed_count_clone = processed_count.clone();
        let skipped_count_clone = skipped_count.clone();
        let file_paths_clone = file_paths.clone();
        let path_clone = path.clone();

        // Spawn a task for each file
        tasks.spawn(async move {
            // Acquire a permit from the semaphore
            let _permit = semaphore_clone.acquire().await.unwrap();

            // Process the file
            match process_single_file_for_embedding(&path_clone, &config_clone, &client_clone).await
            {
                Ok(embedding) => {
                    // Successfully processed
                    let count = processed_count_clone.fetch_add(1, Ordering::SeqCst) + 1;

                    // Store result
                    let mut metadata = file_paths_clone.lock().unwrap();
                    metadata.push((file_id, path_clone, embedding));

                    // Show progress periodically
                    if count.is_multiple_of(PROGRESS_INTERVAL) {
                        println!("Processed {count} files…");
                    }

                    Ok(())
                }
                Err(e) => {
                    // Log error and count as skipped
                    eprintln!(
                        "Warning: Failed to process file {}: {}",
                        path_clone.display(),
                        e
                    );
                    skipped_count_clone.fetch_add(1, Ordering::SeqCst);
                    Err(e)
                }
            }
        });
    }

    // Wait for all tasks to complete
    while let Some(result) = tasks.join_next().await {
        // Propagate both join errors and any task errors
        result??;
    }

    // Update stats from atomic counters
    stats.files_processed += processed_count.load(Ordering::SeqCst);
    stats.files_skipped += skipped_count.load(Ordering::SeqCst);

    // Sort results by file_id and insert into the index
    let mut results = file_paths.lock().unwrap();
    results.sort_by_key(|(id, _, _)| *id);

    // Now populate the index and metadata
    for (_, path, embedding) in results.iter() {
        let file_id = file_metadata.len();
        index.insert((embedding.as_slice(), file_id));
        file_metadata.push(path.clone());
    }

    println!("Successfully indexed {} files", file_metadata.len());
    Ok(())
}

/// Determines if a path should be skipped during traversal.
fn should_skip_path(path: &std::path::Path) -> bool {
    // Skip either directories that match our skip patterns
    // or any path that ends with these patterns (like .git)
    SKIP_DIRS.iter().any(|&dir| path.ends_with(dir))
}

/// Checks if a file has a supported extension for indexing.
fn is_supported_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SUPPORTED_EXTENSIONS.contains(&ext))
}

/// Processes a single file for embedding without modifying the index.
///
/// This function handles just the embedding part, making it suitable for
/// parallel processing in our async pipeline.
///
/// # Arguments
/// * `path` - Path to the file being processed
/// * `config` - Configuration settings for ingestion
/// * `client` - HTTP client for embedding API requests
///
/// # Returns
/// The embedding vector on success
///
/// # Errors
/// Returns error if file reading or embedding generation fails.
async fn process_single_file_for_embedding(
    path: &std::path::Path,
    config: &IngestConfig,
    client: &reqwest::Client,
) -> Result<Vec<f32>> {
    // Read and truncate file content
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let truncated_content = truncate_content(&content, config.max_chars);

    // Generate embedding vector
    let embedding = embed::embed(truncated_content, config.max_tokens, client)
        .await
        .with_context(|| format!("Failed to generate embedding for file: {}", path.display()))?;

    Ok(embedding)
}

/// Processes a single file and adds it to the index.
///
/// # Arguments
/// * `path` - Path to the file being processed
/// * `config` - Configuration settings for ingestion
/// * `client` - HTTP client for embedding API requests
/// * `index` - HNSW index to insert embeddings into
/// * `file_metadata` - Collection of file paths to track processed files
/// * `file_id` - Unique identifier for this file in the index
///
/// # Returns
/// Success if the file was processed and added to the index.
///
/// # Errors
/// Returns error if file reading or embedding generation fails.
///
#[allow(dead_code)]
/// @deprecated Use the parallel processing pipeline instead
async fn process_single_file(
    path: &std::path::Path,
    config: &IngestConfig,
    client: &reqwest::Client,
    index: &Hnsw<'_, f32, DistCosine>,
    file_metadata: &mut Vec<PathBuf>,
    file_id: usize,
) -> Result<()> {
    // Generate embedding
    let embedding = process_single_file_for_embedding(path, config, client).await?;

    // Insert into index
    index.insert((embedding.as_slice(), file_id));

    // Store file path metadata
    file_metadata.push(path.to_path_buf());

    Ok(())
}

/// Truncates content to the specified maximum length.
fn truncate_content(content: &str, max_chars: usize) -> &str {
    if content.len() <= max_chars {
        content
    } else {
        &content[..max_chars]
    }
}

/// Persists the HNSW index and file metadata to disk.
fn persist_index_data(
    index: &Hnsw<'_, f32, DistCosine>,
    file_metadata: &[PathBuf],
    output_dir: &std::path::Path,
) -> Result<()> {
    // Create output directory
    std::fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_dir.display()
        )
    })?;

    // Save HNSW index using file_dump method
    let file_basename = "index";
    index
        .file_dump(output_dir, file_basename)
        .map_err(|e| anyhow::anyhow!("Failed to save HNSW index: {}", e))
        .with_context(|| {
            format!(
                "Failed to save HNSW index to: {}/{}",
                output_dir.display(),
                file_basename
            )
        })?;

    // Save metadata as JSON
    let metadata_path = output_dir.join("meta.json");
    let metadata_file = File::create(&metadata_path).with_context(|| {
        format!(
            "Failed to create metadata file: {}",
            metadata_path.display()
        )
    })?;

    serde_json::to_writer(metadata_file, &json!(file_metadata))
        .with_context(|| format!("Failed to write metadata to: {}", metadata_path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_should_skip_path() {
        assert!(should_skip_path(Path::new(".git")));
        assert!(should_skip_path(Path::new("project/.git")));
        assert!(should_skip_path(Path::new("target")));
        assert!(!should_skip_path(Path::new("src")));
        assert!(!should_skip_path(Path::new("README.md")));
    }

    #[test]
    fn test_is_supported_file() {
        assert!(is_supported_file(Path::new("README.md")));
        assert!(is_supported_file(Path::new("config.json")));
        assert!(!is_supported_file(Path::new("binary.exe")));
        assert!(!is_supported_file(Path::new("script.py")));
    }

    #[test]
    fn test_truncate_content() {
        let long_content = "a".repeat(1000);
        assert_eq!(truncate_content(&long_content, 500).len(), 500);

        let short_content = "short";
        assert_eq!(truncate_content(short_content, 500), "short");
    }

    #[test]
    fn test_ingest_config_default() {
        let config = IngestConfig::default();
        assert_eq!(config.max_chars, MAX_FILE_CHARS);
        assert_eq!(config.max_tokens, MAX_EMBEDDING_TOKENS);
    }
}
