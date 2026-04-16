#![allow(dead_code)]

#[allow(unused_imports)]
use anyhow::{Context, Result};
#[cfg(feature = "cli")]
use clap::{Arg, ArgAction, Command};
use colored::*;
use std::fs;
#[allow(unused_imports)]
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Configuration for validation
#[derive(Debug)]
#[allow(dead_code)]
struct ValidatorConfig {
    fix_mode: bool,
    verbose: bool,
    target_path: PathBuf,
}

/// Validation issue type
#[derive(Debug)]
#[allow(dead_code)]
struct NamingIssue {
    path: PathBuf,
    issue_type: String,
    suggested_fix: String,
}

/// Log utilities with colored output
#[allow(dead_code)]
fn log_info(message: &str) {
    println!("{} {}", "[INFO]".blue().bold(), message);
}

fn log_success(message: &str) {
    println!("{} {}", "[SUCCESS]".green().bold(), message);
}

fn log_warning(message: &str) {
    println!("{} {}", "[WARNING]".yellow().bold(), message);
}

fn log_error(message: &str) {
    eprintln!("{} {}", "[ERROR]".red().bold(), message);
}

/// Directory name validation
fn validate_directory_names(config: &ValidatorConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();

    if config.verbose {
        log_info(&format!(
            "Validating directory names in {:?}",
            config.target_path
        ));
    }

    // Exclude patterns
    let excluded = ["node_modules", "target", ".git", ".vscode", "build", "dist"];

    for entry in WalkDir::new(&config.target_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            !excluded.iter().any(|x| {
                e.path()
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy() == *x)
            })
        })
    {
        let entry = entry?;
        if !entry.file_type().is_dir() || entry.path() == config.target_path {
            continue;
        }

        let dirname = entry.file_name().to_string_lossy();

        // Check for snake_case instead of kebab-case
        if dirname.contains('_') {
            let new_name = dirname.replace('_', "-");
            issues.push(NamingIssue {
                path: entry.path().to_path_buf(),
                issue_type: "Directory uses snake_case instead of kebab-case".to_string(),
                suggested_fix: new_name,
            });
        }

        // Check for PascalCase (except in ai-entities)
        if dirname.chars().any(char::is_uppercase)
            && !entry.path().to_string_lossy().contains("/ai-entities")
        {
            let new_name = dirname
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i > 0 && c.is_uppercase() {
                        format!("-{}", c.to_lowercase())
                    } else {
                        c.to_lowercase().to_string()
                    }
                })
                .collect::<String>();
            issues.push(NamingIssue {
                path: entry.path().to_path_buf(),
                issue_type: "Directory uses PascalCase instead of kebab-case".to_string(),
                suggested_fix: new_name.trim_start_matches('-').to_string(),
            });
        }
    }

    if issues.is_empty() {
        if config.verbose {
            log_success("All directory names follow conventions");
        }
    } else {
        log_warning(&format!("Found {} directory naming issues", issues.len()));
    }

    Ok(issues)
}

/// Rust file name validation
fn validate_rust_file_names(config: &ValidatorConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();

    if config.verbose {
        log_info(&format!(
            "Validating Rust file names in {:?}",
            config.target_path
        ));
    }

    for entry in WalkDir::new(&config.target_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() || !entry.path().to_string_lossy().ends_with(".rs") {
            continue;
        }

        let filename = entry
            .path()
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        // Skip special files
        if filename == "main" || filename == "lib" {
            continue;
        }

        // Check for kebab-case instead of snake_case
        if filename.contains('-') {
            let new_name = filename.replace('-', "_");
            issues.push(NamingIssue {
                path: entry.path().to_path_buf(),
                issue_type: "Rust file uses kebab-case instead of snake_case".to_string(),
                suggested_fix: format!("{}.rs", new_name),
            });
        }

        // Check for PascalCase/camelCase
        if filename.chars().any(char::is_uppercase) {
            let new_name = filename
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i > 0 && c.is_uppercase() {
                        format!("_{}", c.to_lowercase())
                    } else {
                        c.to_lowercase().to_string()
                    }
                })
                .collect::<String>();
            issues.push(NamingIssue {
                path: entry.path().to_path_buf(),
                issue_type: "Rust file uses PascalCase/camelCase instead of snake_case".to_string(),
                suggested_fix: format!("{}.rs", new_name.trim_start_matches('_')),
            });
        }
    }

    if issues.is_empty() {
        if config.verbose {
            log_success("All Rust file names follow conventions");
        }
    } else {
        log_warning(&format!("Found {} Rust naming issues", issues.len()));
    }

    Ok(issues)
}

/// Markdown file name validation
fn validate_markdown_file_names(config: &ValidatorConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();

    if config.verbose {
        log_info(&format!(
            "Validating Markdown file names in {:?}",
            config.target_path
        ));
    }

    for entry in WalkDir::new(&config.target_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() || !entry.path().to_string_lossy().ends_with(".md") {
            continue;
        }

        let filename = entry
            .path()
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        // Skip special files (all uppercase)
        if filename.chars().all(|c| c.is_uppercase() || c == '_') {
            continue;
        }

        // Handle entity files differently
        if entry.path().to_string_lossy().contains("/ai-entities/")
            && !filename.contains('-')
            && !filename.contains('_')
        {
            // Entity files should be lowercase
            if filename.chars().any(char::is_uppercase) {
                issues.push(NamingIssue {
                    path: entry.path().to_path_buf(),
                    issue_type: "Entity markdown file should use lowercase".to_string(),
                    suggested_fix: format!("{}.md", filename.to_lowercase()),
                });
            }
        } else {
            // Regular documentation should use kebab-case
            if filename.contains('_') {
                let new_name = filename.replace('_', "-");
                issues.push(NamingIssue {
                    path: entry.path().to_path_buf(),
                    issue_type: "Markdown file uses snake_case instead of kebab-case".to_string(),
                    suggested_fix: format!("{}.md", new_name),
                });
            }
        }
    }

    if issues.is_empty() {
        if config.verbose {
            log_success("All Markdown file names follow conventions");
        }
    } else {
        log_warning(&format!("Found {} Markdown naming issues", issues.len()));
    }

    Ok(issues)
}

/// JSON file name validation
fn validate_json_file_names(config: &ValidatorConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();

    if config.verbose {
        log_info(&format!(
            "Validating JSON file names in {:?}",
            config.target_path
        ));
    }

    for entry in WalkDir::new(&config.target_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() || !entry.path().to_string_lossy().ends_with(".json") {
            continue;
        }

        let filename = entry
            .path()
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        // Skip dot files
        if filename.starts_with('.') {
            continue;
        }

        let path_str = entry.path().to_string_lossy();

        // Entity and personality files should use TitleCase
        if path_str.contains("/personality-archetypes/") || filename.contains("Registry") {
            if !filename
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                let new_name = filename
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            c.to_uppercase().next().unwrap_or(c)
                        } else {
                            c
                        }
                    })
                    .collect::<String>();
                issues.push(NamingIssue {
                    path: entry.path().to_path_buf(),
                    issue_type: "Personality/entity JSON file should use TitleCase".to_string(),
                    suggested_fix: format!("{}.json", new_name),
                });
            }
        // Schema files should use kebab-case
        } else if (path_str.contains("/data/schemas/")
            || path_str.contains("/data/vector")
            || path_str.ends_with("-config.json")
            || path_str.contains("/config/schemas/"))
            && filename.contains('_')
        {
            let new_name = filename.replace('_', "-");
            issues.push(NamingIssue {
                path: entry.path().to_path_buf(),
                issue_type: "Schema/config JSON file should use kebab-case".to_string(),
                suggested_fix: format!("{}.json", new_name),
            });
        }
    }

    if issues.is_empty() {
        if config.verbose {
            log_success("All JSON file names follow conventions");
        }
    } else {
        log_warning(&format!("Found {} JSON naming issues", issues.len()));
    }

    Ok(issues)
}

/// Process and optionally fix naming issues
fn process_issues(config: &ValidatorConfig, issues: &[NamingIssue]) -> Result<()> {
    if issues.is_empty() {
        return Ok(());
    }

    for issue in issues {
        log_warning(&format!("{}: {}", issue.issue_type, issue.path.display()));
        if config.fix_mode {
            println!("  Suggested fix: {}", issue.suggested_fix);
            print!("  Apply this fix? [y/N] ");
            std::io::Write::flush(&mut std::io::stdout())?;

            let mut response = String::new();
            std::io::stdin().read_line(&mut response)?;

            if response.trim().eq_ignore_ascii_case("y") {
                let new_path = issue.path.with_file_name(&issue.suggested_fix);
                fs::rename(&issue.path, &new_path)?;
                log_success(&format!("Renamed to {}", new_path.display()));
            }
        }
    }

    Ok(())
}

/// Main validation function
fn run_validations(config: &ValidatorConfig) -> Result<bool> {
    println!("Validating naming conventions in {:?}", config.target_path);
    println!("========================================");

    let mut success = true;

    // Run all validations
    let dir_issues = validate_directory_names(config)?;
    let rust_issues = validate_rust_file_names(config)?;
    let md_issues = validate_markdown_file_names(config)?;
    let json_issues = validate_json_file_names(config)?;

    // Process issues
    process_issues(config, &dir_issues)?;
    process_issues(config, &rust_issues)?;
    process_issues(config, &md_issues)?;
    process_issues(config, &json_issues)?;

    let total_issues = dir_issues.len() + rust_issues.len() + md_issues.len() + json_issues.len();

    println!("========================================");
    if total_issues == 0 {
        log_success("All naming conventions validated successfully!");
        if config.verbose {
            println!();
            println!("Naming conventions reference:");
            println!("- Directories: kebab-case (e.g., vector-search)");
            println!("- Rust files:  snake_case (e.g., embed.rs)");
            println!("- Markdown:    kebab-case for docs (e.g., character-based-chunking.md)");
            println!("              lowercase for entities (e.g., harald.md)");
            println!(
                "- JSON:        kebab-case for config/schema (e.g., vector-stores-registry.json)"
            );
            println!("              TitleCase for entities/personalities (e.g., Heralds.json)");
        }
    } else {
        log_warning(&format!("Found {} naming convention issues", total_issues));
        println!();
        println!("For more information on naming conventions, see:");
        println!("docs/naming-conventions.md");
        success = false;
    }

    Ok(success)
}

#[cfg(feature = "cli")]
fn main() -> Result<()> {
    let matches = Command::new("validate_naming")
        .about("Validates file and directory naming against project conventions")
        .arg(
            Arg::new("fix")
                .long("fix")
                .help("Suggest and optionally apply fixes for naming issues")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .help("Show detailed information about checks")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("path")
                .help("Path to check (defaults to current directory)")
                .default_value("."),
        )
        .get_matches();

    // Simply use the path argument as provided
    let target_path = PathBuf::from(matches.get_one::<String>("path").unwrap());

    let config = ValidatorConfig {
        fix_mode: matches.get_flag("fix"),
        verbose: matches.get_flag("verbose"),
        target_path,
    };

    match run_validations(&config)? {
        true => Ok(()),
        false => std::process::exit(1),
    }
}
