//! CLI binary for validating naming conventions
//!
//! This binary provides a command-line interface for the naming validation
//! functionality. It uses the library functions from the validation module.

use anyhow::Result;
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
use std::path::PathBuf;

// Import the library functions
use harald::utils::validation::naming::{
    validate_naming_conventions, NamingIssue, ValidationConfig, ValidationResult,
};

fn main() -> Result<()> {
    let matches = Command::new("validate_naming")
        .about("Validates naming conventions for HARALD project files")
        .arg(
            Arg::new("fix")
                .long("fix")
                .help("Automatically fix naming issues")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Show detailed information")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .help("Path to validate (defaults to current directory)")
                .value_name("PATH"),
        )
        .get_matches();

    let config = ValidationConfig {
        target_path: matches
            .get_one::<String>("path")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        fix_issues: matches.get_flag("fix"),
        verbose: matches.get_flag("verbose"),
    };

    // Run validation
    let result = validate_naming_conventions(&config)?;

    // Display results
    display_results(&result, &config);

    // Exit with appropriate code
    if result.issues.is_empty() {
        log_success("All naming conventions are compliant!");
        Ok(())
    } else if config.fix_issues && result.fixed_count > 0 {
        log_info(&format!(
            "Fixed {} issues, {} errors occurred",
            result.fixed_count, result.error_count
        ));
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn display_results(result: &ValidationResult, config: &ValidationConfig) {
    if result.issues.is_empty() {
        return;
    }

    log_warning(&format!(
        "Found {} naming convention issues:",
        result.issues.len()
    ));
    println!();

    for issue in &result.issues {
        display_issue(issue, config.verbose);
    }

    if !config.fix_issues {
        println!();
        log_info("Run with --fix to automatically correct these issues");
    }
}

fn display_issue(issue: &NamingIssue, verbose: bool) {
    let issue_type = match issue.issue_type {
        harald::utils::validation::naming::IssueType::DirectoryNaming => "Directory",
        harald::utils::validation::naming::IssueType::RustFileNaming => "Rust File",
        harald::utils::validation::naming::IssueType::MarkdownFileNaming => "Markdown File",
        harald::utils::validation::naming::IssueType::JsonFileNaming => "JSON File",
    };

    println!(
        "  {} {}",
        format!("[{}]", issue_type).cyan().bold(),
        issue.path.display()
    );

    println!("    {}: {}", "Current".red(), issue.current_name);

    println!("    {}: {}", "Suggested".green(), issue.suggested_name);

    if verbose {
        println!("    {}: {}", "Reason".yellow(), issue.description);
    }

    println!();
}

// Logging utilities
fn log_info(message: &str) {
    println!("{} {}", "[INFO]".blue().bold(), message);
}

fn log_success(message: &str) {
    println!("{} {}", "[SUCCESS]".green().bold(), message);
}

fn log_warning(message: &str) {
    println!("{} {}", "[WARNING]".yellow().bold(), message);
}

#[allow(dead_code)]
fn log_error(message: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), message);
}
