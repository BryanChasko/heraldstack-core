//! Single Character Ingest CLI Binary
//!
//! Command-line interface for testing single character ingestion.
//! This is a temporary implementation that will be refactored later.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about = "Single Character Ingest Test", long_about = None)]
struct Args {
    /// Path to the single character JSON file (array of objects)
    #[arg(
        short,
        long,
        help = "Path to the single character JSON file (array of objects)"
    )]
    input: std::path::PathBuf,
}

fn main() {
    println!("❌ Single character ingest CLI is currently being refactored.");
    println!("   This tool is temporarily disabled during the separation of concerns migration.");
    println!("   Use the library function directly or wait for the refactoring to complete.");

    let args = Args::parse();
    println!("   Input file specified: {}", args.input.display());

    std::process::exit(1);
}
