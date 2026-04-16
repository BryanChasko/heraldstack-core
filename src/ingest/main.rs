//! Main entry point for the HARALD semantic search and RAG system.
//!
//! This binary provides a command-line interface for:
//! - Ingesting files into a searchable vector index
//! - Querying the index for semantic search and RAG responses
//!
//! The application uses the flat module structure where each .rs file
//! defines a module loaded via `mod module_name;` declarations.

use anyhow::Result;
use clap::{Parser, Subcommand};

// Use the proper module structure from lib.rs
use harald::ingest::{query, runner, QueryConfig};

/// Command-line interface for HARALD semantic search system.
///
/// This tool provides semantic search capabilities over your codebase
/// using vector embeddings and retrieval-augmented generation (RAG).
#[derive(Parser)]
#[command(
    author = "HARALD Team",
    version = "0.1.0",
    about = "Semantic search and RAG for the HeraldStack",
    long_about = "A Rust-based semantic search system that ingests documentation \
                  and provides intelligent query responses using local language models."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available commands for the HARALD system.
#[derive(Subcommand)]
enum Commands {
    /// Build HNSW index from local *.md and *.json files.
    ///
    /// This command recursively scans the current directory for Markdown
    /// and JSON files, generates embeddings, and builds a searchable index.
    Ingest {
        /// Root directory to start ingestion from (defaults to current directory)
        #[arg(short, long)]
        root: Option<std::path::PathBuf>,

        /// Maximum characters to read per file
        #[arg(long, default_value = "800")]
        max_chars: usize,

        /// Maximum tokens for embedding requests
        #[arg(long, default_value = "600")]
        max_tokens: usize,

        /// Maximum number of files to process concurrently
        /// If not specified, defaults to number of CPU cores
        #[arg(long)]
        max_concurrent: Option<usize>,
    },

    /// Ask a question using the pre-built index.
    ///
    /// This command performs semantic search to find relevant documents
    /// and uses them as context for generating responses via a local LLM.
    Query {
        /// The question or search query
        #[arg(required = true)]
        prompt: Vec<String>,

        /// Root directory containing the data folder with index files (defaults to current directory)
        #[arg(short, long)]
        root: Option<std::path::PathBuf>,

        /// Number of similar documents to retrieve for context
        #[arg(short, long, default_value = "3")]
        num_results: usize,

        /// Maximum characters to include from each retrieved document
        #[arg(long, default_value = "800")]
        max_context_chars: usize,

        /// Language model endpoint URL
        #[arg(long, default_value = "http://127.0.0.1:11434/api/chat")]
        llm_endpoint: String,

        /// Language model name
        #[arg(long, default_value = "harald-phi4")]
        model_name: String,
    },
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Execute the appropriate command
    match cli.command {
        Commands::Ingest {
            root,
            max_chars,
            max_tokens,
            max_concurrent,
        } => {
            // Create ingest configuration
            let mut config = runner::IngestConfig::default();

            if let Some(root_dir) = root {
                config.root_dir = root_dir;
            }
            config.max_chars = max_chars;
            config.max_tokens = max_tokens;
            config.max_concurrent_files = max_concurrent;

            // Run ingestion process
            let stats = runner::run_with_config(config).await?;

            // Display results
            println!(
                "✅ Ingestion completed successfully!\n\
                 📁 Processed: {} files\n\
                 ⏭️  Skipped: {} files\n\
                 💾 Output: {}",
                stats.files_processed,
                stats.files_skipped,
                stats.output_dir.display()
            );
        }

        Commands::Query {
            prompt,
            root,
            num_results,
            max_context_chars,
            llm_endpoint,
            model_name,
        } => {
            // Join prompt words into a single query string
            let query_text = prompt.join(" ");

            // Create query configuration with all options set at initialization
            let mut config = QueryConfig {
                num_results,
                max_context_chars,
                llm_endpoint,
                model_name,
                ..Default::default()
            };

            // Set root directory if provided
            if let Some(root_dir) = root {
                config.root_dir = root_dir;
            }

            // Execute query
            let result = query::run_with_config(&query_text, config).await?;

            // Display results
            println!("🔍 Query: {query_text}");
            println!("📚 Context from {} documents:", result.num_context_docs);
            for (i, file) in result.context_files.iter().enumerate() {
                println!("  {}. {}", i + 1, file.display());
            }
            println!("\n🤖 Response:");
            println!("{}", result.response);
        }
    }

    Ok(())
}
