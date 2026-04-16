//! Naming convention validation utilities
//!
//! This module provides functions for validating file and directory names
//! against HARALD project naming conventions. It can be used both as a
//! library (by other modules) and via the CLI binary.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Types of naming validation issues
#[derive(Debug, Clone)]
pub enum IssueType {
    DirectoryNaming,
    RustFileNaming,
    MarkdownFileNaming,
    JsonFileNaming,
}

/// A naming convention issue found during validation
#[derive(Debug, Clone)]
pub struct NamingIssue {
    pub path: PathBuf,
    pub issue_type: IssueType,
    pub current_name: String,
    pub suggested_name: String,
    pub description: String,
}

/// Configuration for naming validation
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub target_path: PathBuf,
    pub fix_issues: bool,
    pub verbose: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            target_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            fix_issues: false,
            verbose: false,
        }
    }
}

/// Result of a validation run
#[derive(Debug)]
pub struct ValidationResult {
    pub issues: Vec<NamingIssue>,
    pub fixed_count: usize,
    pub error_count: usize,
}

/// Main validation function - validates all naming conventions
pub fn validate_naming_conventions(config: &ValidationConfig) -> Result<ValidationResult> {
    let mut all_issues = Vec::new();
    let mut fixed_count = 0;
    let mut error_count = 0;

    // Collect all validation issues
    all_issues.extend(validate_directory_names(config)?);
    all_issues.extend(validate_rust_file_names(config)?);
    all_issues.extend(validate_markdown_file_names(config)?);
    all_issues.extend(validate_json_file_names(config)?);

    // Apply fixes if requested
    if config.fix_issues {
        for issue in &all_issues {
            match apply_fix(issue) {
                Ok(()) => fixed_count += 1,
                Err(_) => error_count += 1,
            }
        }
    }

    Ok(ValidationResult {
        issues: all_issues,
        fixed_count,
        error_count,
    })
}

/// Validate directory naming conventions
pub fn validate_directory_names(config: &ValidationConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();
    let excluded = ["node_modules", "target", ".git", ".vscode", "build", "dist"];

    for entry in WalkDir::new(&config.target_path)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| {
            !excluded
                .iter()
                .any(|x| e.file_name().to_string_lossy().contains(x))
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        let path = entry.path();
        let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip root directory
        if path == config.target_path {
            continue;
        }

        // Check for kebab-case compliance
        if !is_valid_kebab_case(dir_name) && !is_special_directory(dir_name) {
            issues.push(NamingIssue {
                path: path.to_path_buf(),
                issue_type: IssueType::DirectoryNaming,
                current_name: dir_name.to_string(),
                suggested_name: to_kebab_case(dir_name),
                description: format!("Directory '{}' should use kebab-case", dir_name),
            });
        }
    }

    Ok(issues)
}

/// Validate Rust file naming conventions
pub fn validate_rust_file_names(config: &ValidationConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();

    for entry in WalkDir::new(&config.target_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let path = entry.path();
        let file_stem = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");

        // Skip special Rust files
        if ["main", "lib", "mod"].contains(&file_stem) {
            continue;
        }

        if !is_valid_snake_case(file_stem) {
            issues.push(NamingIssue {
                path: path.to_path_buf(),
                issue_type: IssueType::RustFileNaming,
                current_name: file_stem.to_string(),
                suggested_name: to_snake_case(file_stem),
                description: format!("Rust file '{}' should use snake_case", file_stem),
            });
        }
    }

    Ok(issues)
}

/// Validate Markdown file naming conventions  
pub fn validate_markdown_file_names(config: &ValidationConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();

    for entry in WalkDir::new(&config.target_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let path = entry.path();
        let file_stem = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");

        // Check naming convention based on context
        let expected_case = if is_entity_file(path) {
            "TitleCase"
        } else if is_standard_doc(file_stem) {
            "UPPERCASE"
        } else {
            "kebab-case"
        };

        let is_valid = match expected_case {
            "TitleCase" => is_valid_title_case(file_stem),
            "UPPERCASE" => file_stem
                .chars()
                .all(|c| c.is_uppercase() || c == '-' || c == '_'),
            "kebab-case" => is_valid_kebab_case(file_stem),
            _ => false,
        };

        if !is_valid {
            let suggested = match expected_case {
                "TitleCase" => to_title_case(file_stem),
                "UPPERCASE" => file_stem.to_uppercase(),
                "kebab-case" => to_kebab_case(file_stem),
                _ => file_stem.to_string(),
            };

            issues.push(NamingIssue {
                path: path.to_path_buf(),
                issue_type: IssueType::MarkdownFileNaming,
                current_name: file_stem.to_string(),
                suggested_name: suggested,
                description: format!("Markdown file '{}' should use {}", file_stem, expected_case),
            });
        }
    }

    Ok(issues)
}

/// Validate JSON file naming conventions
pub fn validate_json_file_names(config: &ValidationConfig) -> Result<Vec<NamingIssue>> {
    let mut issues = Vec::new();

    for entry in WalkDir::new(&config.target_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
    {
        let path = entry.path();
        let file_stem = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");

        let expected_case = if is_entity_or_archetype_file(path) {
            "TitleCase"
        } else if is_config_file(path) {
            "kebab-case"
        } else {
            "snake_case"
        };

        let is_valid = match expected_case {
            "TitleCase" => is_valid_title_case(file_stem),
            "kebab-case" => is_valid_kebab_case(file_stem),
            "snake_case" => is_valid_snake_case(file_stem),
            _ => false,
        };

        if !is_valid {
            let suggested = match expected_case {
                "TitleCase" => to_title_case(file_stem),
                "kebab-case" => to_kebab_case(file_stem),
                "snake_case" => to_snake_case(file_stem),
                _ => file_stem.to_string(),
            };

            issues.push(NamingIssue {
                path: path.to_path_buf(),
                issue_type: IssueType::JsonFileNaming,
                current_name: file_stem.to_string(),
                suggested_name: suggested,
                description: format!("JSON file '{}' should use {}", file_stem, expected_case),
            });
        }
    }

    Ok(issues)
}

// Helper functions for case validation and conversion

fn is_valid_kebab_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_lowercase() || c.is_numeric() || c == '-')
}

fn is_valid_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_lowercase() || c.is_numeric() || c == '_')
}

fn is_valid_title_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().unwrap().is_uppercase()
        && s.chars().skip(1).all(|c| c.is_alphanumeric())
}

fn to_kebab_case(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn to_snake_case(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn to_title_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in s.chars() {
        if c.is_alphabetic() {
            if capitalize_next {
                result.push(c.to_uppercase().next().unwrap());
                capitalize_next = false;
            } else {
                result.push(c.to_lowercase().next().unwrap());
            }
        } else if c.is_numeric() {
            result.push(c);
        }
        // Skip non-alphanumeric characters
    }

    result
}

fn is_special_directory(name: &str) -> bool {
    ["src", "tests", "docs", "scripts", "config", "data", "logs"].contains(&name)
}

fn is_entity_file(path: &Path) -> bool {
    path.ancestors()
        .any(|p| p.file_name().is_some_and(|n| n == "ai-entities"))
}

fn is_standard_doc(name: &str) -> bool {
    ["README", "CONTRIBUTING", "LICENSE", "CHANGELOG"].contains(&name)
}

fn is_entity_or_archetype_file(path: &Path) -> bool {
    path.ancestors().any(|p| {
        p.file_name()
            .is_some_and(|n| n == "ai-entities" || n == "personality-archetypes")
    })
}

fn is_config_file(path: &Path) -> bool {
    path.ancestors()
        .any(|p| p.file_name().is_some_and(|n| n == "config"))
}

fn apply_fix(issue: &NamingIssue) -> Result<()> {
    let old_path = &issue.path;
    let new_path = old_path
        .with_file_name(&issue.suggested_name)
        .with_extension(old_path.extension().unwrap_or_default());

    std::fs::rename(old_path, &new_path)
        .with_context(|| format!("Failed to rename {:?} to {:?}", old_path, new_path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_validation() {
        assert!(is_valid_kebab_case("hello-world"));
        assert!(!is_valid_kebab_case("HelloWorld"));

        assert!(is_valid_snake_case("hello_world"));
        assert!(!is_valid_snake_case("hello-world"));

        assert!(is_valid_title_case("HelloWorld"));
        assert!(!is_valid_title_case("hello-world"));
    }

    #[test]
    fn test_case_conversion() {
        assert_eq!(to_kebab_case("HelloWorld"), "hello-world");
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
        assert_eq!(to_title_case("hello-world"), "HelloWorld");
    }
}
