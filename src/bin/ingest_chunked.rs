//! Chunked Ingest CLI Binary
//!
//! Command-line interface for character-based chunking ingestion.
//! This binary uses the library functions from the ingest module.

use anyhow::Result;
use clap::{Arg, Command};
use harald::ingest::chunked_ingest::{process_file, ChunkedIngestConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("ingest_chunked")
        .about("Character-based chunking for Marvel character data")
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .value_name("FILE")
                .help("JSON file to process")
                .default_value(
                    "/Users/bryanchasko/Code/HARALD/tests/fixtures/test_single_character.json",
                ),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .value_name("MODEL")
                .help("Ollama model to use for embeddings")
                .default_value("harald-phi4"),
        )
        .get_matches();

    let file_path = matches.get_one::<String>("file").unwrap();
    let model = matches.get_one::<String>("model").unwrap();

    println!("🚀 Starting chunked ingestion process...");
    println!("   File: {}", file_path);
    println!("   Model: {}", model);

    let config = ChunkedIngestConfig {
        model_name: model.to_string(),
        max_chunk_size: 250,
        ..Default::default()
    };

    match process_file(file_path, &config).await {
        Ok(result) => {
            println!("\n✅ Chunked ingestion completed successfully!");
            println!("   Characters processed: {}", result.characters_processed);
            println!("   Chunks created: {}", result.chunks_created);
            println!("   Embeddings generated: {}", result.embeddings_generated);
            println!("   Processing time: {:.2}s", result.processing_time_secs);
        }
        Err(e) => {
            eprintln!("❌ Chunked ingestion failed: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
