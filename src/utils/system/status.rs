#![allow(dead_code)]

use anyhow::{Context, Result};
#[cfg(feature = "cli")]
use clap::{Arg, ArgAction, Command};
use colored::Colorize;
#[allow(unused_imports)]
use std::process::{Command as ProcessCommand, Output};
#[allow(unused_imports)]
use std::time::{Duration, Instant};

/// Configuration for status check
#[derive(Debug)]
struct StatusConfig {
    verbose: bool,
    check_all: bool,
}

fn log_success(message: &str) {
    println!("{} {}", "✅".green(), message);
}

fn log_error(message: &str) {
    println!("{} {}", "❌".red(), message);
}

fn log_info(message: &str) {
    println!("{} {}", "🔍".blue(), message);
}

fn log_warning(message: &str) {
    println!("{} {}", "⚠️".yellow(), message);
}

/// Check if Ollama is running
fn check_ollama(config: &StatusConfig) -> Result<bool> {
    log_info("Checking Ollama service...");

    // Check if process is running
    let output = ProcessCommand::new("pgrep")
        .arg("-x")
        .arg("ollama")
        .output()
        .context("Failed to execute pgrep to check if Ollama is running")?;

    if output.status.success() {
        // Get PID from output
        let pid = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Get memory usage
        let mem_output = ProcessCommand::new("ps")
            .arg("-o")
            .arg("rss=")
            .arg("-p")
            .arg(&pid)
            .output()
            .context("Failed to get memory usage")?;

        let mem_kb = String::from_utf8_lossy(&mem_output.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        let mem_gb = mem_kb / 1024.0 / 1024.0;

        log_success("Ollama service: RUNNING");
        println!("   Memory usage: {:.2} GB", mem_gb);

        // Check API connectivity
        match ProcessCommand::new("curl")
            .arg("-s")
            .arg("http://localhost:11434/api/version")
            .output()
        {
            Ok(version_output) if version_output.status.success() => {
                let version_str = String::from_utf8_lossy(&version_output.stdout);
                if let Some(version) = extract_version(&version_str) {
                    println!("   Version: {}", version);
                }

                // Try embedding test
                let start = Instant::now();
                let embed_test = check_embedding_api(config)?;
                let duration = start.elapsed();

                if embed_test {
                    log_success(&format!(
                        "Embedding API: WORKING (response time: {:.2}s)",
                        duration.as_secs_f64()
                    ));
                } else {
                    log_error("Embedding API: NOT WORKING");
                }
            }
            _ => {
                log_error("Ollama API: NOT RESPONDING");
            }
        }

        Ok(true)
    } else {
        log_error("Ollama service: NOT RUNNING");
        println!("   Run 'ollama serve' to start the service");
        Ok(false)
    }
}

/// Extract version from API response
fn extract_version(response: &str) -> Option<String> {
    let parts: Vec<&str> = response.split(r#""version":"#).collect();
    if parts.len() < 2 {
        return None;
    }

    let version_part = parts[1];
    let end_idx = version_part.find('"').unwrap_or(0);
    if end_idx == 0 {
        return None;
    }

    Some(version_part[..end_idx].to_string())
}

/// Check if the embedding API is working
fn check_embedding_api(config: &StatusConfig) -> Result<bool> {
    let output = ProcessCommand::new("curl")
        .arg("-s")
        .arg("-X")
        .arg("POST")
        .arg("http://localhost:11434/api/embeddings")
        .arg("-d")
        .arg(r#"{"model":"harald-phi4","prompt":"test"}"#)
        .output()
        .context("Failed to test embedding API")?;

    let response = String::from_utf8_lossy(&output.stdout);

    if config.verbose {
        println!("Embedding API Response: {}", response);
    }

    Ok(response.contains("embedding"))
}

/// Check models available in Ollama
fn check_models(config: &StatusConfig) -> Result<()> {
    log_info("Checking available models...");

    let output = ProcessCommand::new("ollama")
        .arg("list")
        .output()
        .context("Failed to list Ollama models")?;

    let models = String::from_utf8_lossy(&output.stdout);

    if config.verbose {
        println!("\nModels available:");
        println!("{}", models);
    }

    // Check for required models
    let required_models = ["harald-phi4"];
    for model in required_models {
        if models.contains(model) {
            log_success(&format!("Required model '{}' is available", model));
        } else {
            log_error(&format!("Required model '{}' is NOT available", model));
            println!("   Run 'ollama pull {}' to install", model);
        }
    }

    Ok(())
}

/// Check file system status
#[allow(dead_code)]
fn check_filesystem(_config: &StatusConfig) -> Result<()> {
    log_info("Checking filesystem...");

    // Check disk space
    let output = ProcessCommand::new("df")
        .arg("-h")
        .arg(".")
        .output()
        .context("Failed to check disk space")?;

    let df_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = df_output.lines().collect();

    if lines.len() >= 2 {
        let parts: Vec<&str> = lines[1].split_whitespace().collect();
        if parts.len() >= 5 {
            let used_percent = parts[4];
            let used_value = used_percent
                .trim_end_matches('%')
                .parse::<u32>()
                .unwrap_or(0);

            if used_value > 90 {
                log_warning(&format!("Disk usage: {} (critical)", used_percent));
            } else if used_value > 80 {
                log_warning(&format!("Disk usage: {} (high)", used_percent));
            } else {
                log_success(&format!("Disk usage: {} (ok)", used_percent));
            }

            println!("   Available space: {}", parts[3]);
        }
    }

    Ok(())
}

/// Check vector store status
fn check_vector_store(config: &StatusConfig) -> Result<()> {
    log_info("Checking vector store status...");

    // For this example, we'll just list the vector stores
    let registry_path = "config/vector-stores-registry.json";

    // Check if registry exists
    let output = ProcessCommand::new("cat").arg(registry_path).output();

    match output {
        Ok(output) if output.status.success() => {
            let registry = String::from_utf8_lossy(&output.stdout);
            log_success("Vector store registry found");

            if config.verbose {
                // Try to count the number of stores
                let store_count = registry.matches(r#""name":"#).count();
                println!("   {} vector stores registered", store_count);
            }
        }
        _ => {
            log_error("Vector store registry not found or cannot be read");
        }
    }

    Ok(())
}

/// Run the status check
fn run_status_check(config: &StatusConfig) -> Result<bool> {
    println!("🔍 Checking HARALD System Status");
    println!("--------------------------------");

    let ollama_running = check_ollama(config)?;

    if ollama_running {
        check_models(config)?;
    }

    if config.check_all {
        check_filesystem(config)?;
        check_vector_store(config)?;
    }

    println!("\nStatus check completed.");

    Ok(ollama_running)
}

#[cfg(feature = "cli")]
fn main() -> Result<()> {
    let matches = Command::new("status")
        .about("Checks the status of HARALD system components")
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Show detailed information")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .short('a')
                .help("Check all subsystems including filesystem and vector stores")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    let config = StatusConfig {
        verbose: matches.get_flag("verbose"),
        check_all: matches.get_flag("all"),
    };

    match run_status_check(&config)? {
        true => Ok(()),
        false => std::process::exit(1),
    }
}
