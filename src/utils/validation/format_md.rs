#![allow(dead_code)]

use anyhow::{Context, Result};
#[cfg(feature = "cli")]
use clap::{Arg, ArgAction, Command};
use colored::*;
use std::process::Command as ProcessCommand;
use walkdir::WalkDir;

/// Configuration for Markdown formatting
#[derive(Debug)]
struct FormatterConfig {
    verbose: bool,
    check_only: bool,
    target_path: String,
}

/// Format Markdown files using prettier
fn format_markdown_files(config: &FormatterConfig) -> Result<bool> {
    println!("Formatting Markdown files in {}", config.target_path);

    if config.verbose {
        println!("{} Finding Markdown files...", "[INFO]".blue().bold());
    }

    // Find all .md files, excluding node_modules and target directories
    let mut md_files = Vec::new();

    for entry in WalkDir::new(&config.target_path)
        .into_iter()
        .filter_entry(|e| {
            let path = e.path().to_string_lossy();
            !path.contains("/node_modules/") && !path.contains("/target/")
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.path().to_string_lossy().ends_with(".md") {
            md_files.push(entry.path().to_string_lossy().into_owned());
        }
    }

    if md_files.is_empty() {
        println!("{} No Markdown files found.", "[INFO]".blue().bold());
        return Ok(true);
    }

    if config.verbose {
        println!(
            "{} Found {} Markdown files",
            "[INFO]".blue().bold(),
            md_files.len()
        );
    }

    // Prepare prettier arguments
    let mut prettier_args = vec![
        "--parser".to_string(),
        "markdown".to_string(),
        "--print-width".to_string(),
        "80".to_string(),
        "--prose-wrap".to_string(),
        "always".to_string(),
        "--log-level".to_string(),
        "warn".to_string(),
    ];

    if config.check_only {
        prettier_args.push("--check".to_string());
    } else {
        prettier_args.push("--write".to_string());
    }

    // Add files to the arguments
    prettier_args.extend(md_files);

    // Run prettier
    if config.verbose {
        println!("{} Running prettier...", "[INFO]".blue().bold());
    }

    let output = ProcessCommand::new("prettier")
        .args(&prettier_args)
        .output()
        .context("Failed to execute prettier. Is it installed?")?;

    let success = output.status.success();

    if success {
        if !config.check_only {
            println!(
                "{} Markdown formatting complete!",
                "[SUCCESS]".green().bold()
            );
        } else {
            println!(
                "{} Markdown files check passed!",
                "[SUCCESS]".green().bold()
            );
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if config.check_only {
            println!(
                "{} Some Markdown files need formatting.",
                "[WARNING]".yellow().bold()
            );
            if config.verbose {
                println!("{}", stderr);
            }
        } else {
            println!(
                "{} Failed to format Markdown files.",
                "[ERROR]".red().bold()
            );
            println!("{}", stderr);
        }
    }

    Ok(success)
}

#[cfg(feature = "cli")]
fn main() -> Result<()> {
    let matches = Command::new("format_md")
        .about("Formats Markdown files using prettier with consistent settings")
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .help("Show detailed information about processing")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("check")
                .long("check")
                .help("Check if files are formatted correctly without modifying them")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("path")
                .help("Path to check (defaults to current directory)")
                .default_value("."),
        )
        .get_matches();

    let config = FormatterConfig {
        verbose: matches.get_flag("verbose"),
        check_only: matches.get_flag("check"),
        target_path: matches.get_one::<String>("path").unwrap().to_string(),
    };

    match format_markdown_files(&config)? {
        true => Ok(()),
        false => std::process::exit(1),
    }
}
