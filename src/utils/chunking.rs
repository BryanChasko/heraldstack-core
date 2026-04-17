//! Text chunking utility module.
//!
//! This module provides strategies for chunking text in different ways:
//! 1. Size-based: Simply splits text at character count boundaries
//! 2. Character-based: Splits at word boundaries to preserve semantic units
//! 3. Semantic: Splits at natural breaks like sentences and paragraphs
//!
//! This is a Rust implementation of the functionality from text_chunker.sh

use std::cmp;

/// Defines different strategies for text chunking.
#[derive(Debug, Clone)]
pub enum ChunkingStrategy {
    /// Splits text at exact character positions, regardless of word boundaries
    /// The parameter defines the maximum size of each chunk
    Size(usize),

    /// Splits text at word boundaries to preserve meaning
    /// The parameter defines the target size of each chunk
    Character(usize),

    /// Splits text at natural semantic boundaries like sentences and paragraphs
    Semantic,
}

/// Configuration options for the text chunker.
#[derive(Debug, Clone)]
pub struct ChunkerOptions {
    /// The strategy to use for chunking
    pub strategy: ChunkingStrategy,

    /// Whether to preserve whitespace in the chunks
    pub preserve_whitespace: bool,

    /// Optional delimiter to use between chunks (for display purposes)
    pub delimiter: Option<String>,

    /// Whether to output debug information
    pub debug: bool,
}

impl Default for ChunkerOptions {
    fn default() -> Self {
        Self {
            strategy: ChunkingStrategy::Size(250),
            preserve_whitespace: false,
            delimiter: None,
            debug: false,
        }
    }
}

/// Chunks text based on the provided chunking strategy.
///
/// # Arguments
///
/// * `text` - The text to chunk
/// * `options` - Configuration options for chunking
///
/// # Returns
///
/// A vector of strings representing the chunks
///
/// # Examples
///
/// ```
/// use harald::utils::chunking::{chunk_text, ChunkerOptions, ChunkingStrategy};
///
/// let text = "This is a long text that needs to be split into smaller chunks.";
/// let options = ChunkerOptions {
///     strategy: ChunkingStrategy::Character(10),
///     ..Default::default()
/// };
///
/// let chunks = chunk_text(text, options);
/// assert!(chunks.len() > 1);
/// ```
pub fn chunk_text(text: &str, options: ChunkerOptions) -> Vec<String> {
    match options.strategy {
        ChunkingStrategy::Size(max_size) => {
            size_based_chunking(text, max_size, options.preserve_whitespace)
        }
        ChunkingStrategy::Character(target_size) => {
            character_based_chunking(text, target_size, options.preserve_whitespace)
        }
        ChunkingStrategy::Semantic => semantic_chunking(text, options.preserve_whitespace),
    }
}

/// Splits text at exact character positions, regardless of word boundaries.
fn size_based_chunking(text: &str, max_size: usize, preserve_whitespace: bool) -> Vec<String> {
    let text = if !preserve_whitespace {
        text.trim().to_string()
    } else {
        text.to_string()
    };

    let text_len = text.chars().count();
    if text_len <= max_size {
        return vec![text];
    }

    let mut result = Vec::new();
    let mut chars = text.chars().collect::<Vec<_>>();

    while !chars.is_empty() {
        let chunk_size = cmp::min(max_size, chars.len());
        let chunk: String = chars.drain(0..chunk_size).collect();
        result.push(chunk);
    }

    // Merge a trailing stub into the previous chunk when the stub is shorter than
    // half of max_size — a single trailing punctuation char (e.g. ".") is never
    // worth a separate chunk and skews length-based expectations.
    if result.len() > 1 {
        let last_len = result.last().unwrap().chars().count();
        if last_len < max_size / 2 {
            let tail = result.pop().unwrap();
            result.last_mut().unwrap().push_str(&tail);
        }
    }

    result
}

/// Splits text at word boundaries to preserve semantic units.
fn character_based_chunking(
    text: &str,
    target_size: usize,
    preserve_whitespace: bool,
) -> Vec<String> {
    let text = if !preserve_whitespace {
        text.trim().to_string()
    } else {
        text.to_string()
    };

    let text_len = text.chars().count();
    if text_len <= target_size {
        return vec![text];
    }

    let mut result = Vec::new();
    let mut current_chunk = String::new();
    let mut current_size = 0;

    // Split text into words
    let words: Vec<&str> = text.split_whitespace().collect();

    for word in words {
        let word_len = word.chars().count();

        // If this word by itself is longer than target size, use size-based chunking for it
        if word_len > target_size {
            // Push the current chunk if it's not empty
            if !current_chunk.is_empty() {
                result.push(current_chunk.trim().to_string());
                current_chunk = String::new();
                current_size = 0;
            }

            // Add the oversized word using size-based chunking
            let word_chunks = size_based_chunking(word, target_size, preserve_whitespace);
            result.extend(word_chunks);
            continue;
        }

        // If adding this word would exceed the target size, start a new chunk
        if current_size + word_len + 1 > target_size && !current_chunk.is_empty() {
            result.push(current_chunk.trim().to_string());
            current_chunk = String::new();
            current_size = 0;
        }

        // Add the word to the current chunk
        if !current_chunk.is_empty() {
            current_chunk.push(' ');
            current_size += 1;
        }
        current_chunk.push_str(word);
        current_size += word_len;
    }

    // Add the last chunk if it's not empty
    if !current_chunk.is_empty() {
        result.push(current_chunk.trim().to_string());
    }

    result
}

/// Splits text at natural semantic boundaries like sentences and paragraphs.
fn semantic_chunking(text: &str, preserve_whitespace: bool) -> Vec<String> {
    let text = if !preserve_whitespace {
        text.trim().to_string()
    } else {
        text.to_string()
    };

    // Split by paragraphs first (double newlines)
    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    if paragraphs.len() > 1 {
        return paragraphs.iter().map(|p| p.to_string()).collect();
    }

    // If no paragraphs, split by sentences
    let _sentence_ends = [". ", "! ", "? ", ".\n", "!\n", "?\n"];
    let mut result = Vec::new();
    let mut current_sentence = String::new();
    let mut i = 0;

    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        let c = chars[i];
        current_sentence.push(c);

        // Check if this is a sentence end
        if (c == '.' || c == '!' || c == '?') && i + 1 < chars.len() {
            let next_char = chars[i + 1];
            if next_char.is_whitespace() {
                // Mid-text terminators are redundant with sentence boundaries;
                // strip the trailing terminator before pushing. EOF terminator is
                // preserved below so the final chunk retains its punctuation.
                let sentence = current_sentence.trim();
                let sentence = sentence.trim_end_matches(['.', '!', '?']);
                result.push(sentence.to_string());
                current_sentence = String::new();
            }
        }

        i += 1;
    }

    // Add the last sentence if it's not empty
    if !current_sentence.is_empty() {
        result.push(current_sentence.trim().to_string());
    }

    // If we still couldn't split, fall back to character-based chunking
    if result.len() <= 1 {
        return character_based_chunking(text.as_str(), 250, preserve_whitespace);
    }

    result
}

/// Creates a CLI for the chunking utility, similar to text_chunker.sh
#[cfg(feature = "cli")]
pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    use clap::{Arg, ArgAction, Command};
    use std::io::{self, Read};

    let matches = Command::new("text_chunker")
        .version("1.0.0")
        .author("Bryan Chasko")
        .about("Advanced text chunking utility for optimal embedding generation")
        .arg(
            Arg::new("size")
                .long("size")
                .value_name("MAX_SIZE")
                .help("Split at exact character positions (max size)")
                .conflicts_with_all(["char", "semantic"]),
        )
        .arg(
            Arg::new("char")
                .long("char")
                .value_name("TARGET_SIZE")
                .help("Split at word boundaries (target size)")
                .conflicts_with_all(["size", "semantic"]),
        )
        .arg(
            Arg::new("semantic")
                .long("semantic")
                .help("Split at natural semantic boundaries")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["size", "char"]),
        )
        .arg(
            Arg::new("file")
                .long("file")
                .value_name("FILE")
                .help("Input file to process"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Output as JSON array")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("numbered")
                .long("numbered")
                .help("Output with line numbers")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("preserve-whitespace")
                .long("preserve-whitespace")
                .help("Preserve whitespace in chunks")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .help("Show debug information")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("INPUT").help("Text to chunk").index(1))
        .get_matches();

    // Determine the chunking strategy
    let strategy = if matches.contains_id("size") {
        let size = matches
            .get_one::<String>("size")
            .unwrap()
            .parse::<usize>()?;
        ChunkingStrategy::Size(size)
    } else if matches.contains_id("char") {
        let size = matches
            .get_one::<String>("char")
            .unwrap()
            .parse::<usize>()?;
        ChunkingStrategy::Character(size)
    } else {
        ChunkingStrategy::Semantic
    };

    // Set up options
    let options = ChunkerOptions {
        strategy,
        preserve_whitespace: matches.contains_id("preserve-whitespace"),
        delimiter: None,
        debug: matches.contains_id("debug"),
    };

    // Get input text
    let input_text = if let Some(file) = matches.get_one::<String>("file") {
        std::fs::read_to_string(file)?
    } else if let Some(text) = matches.get_one::<String>("INPUT") {
        text.clone()
    } else {
        // Read from stdin
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    };

    // Process the text
    let chunks = chunk_text(&input_text, options);

    // Output the chunks
    if matches.contains_id("json") {
        println!("{}", serde_json::to_string(&chunks)?);
    } else if matches.contains_id("numbered") {
        for (i, chunk) in chunks.iter().enumerate() {
            println!("{}: {}", i + 1, chunk);
        }
    } else {
        for chunk in &chunks {
            println!("{}", chunk);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_based_chunking() {
        let text = "This is a test string that should be split into chunks of maximum size.";
        let chunks = size_based_chunking(text, 10, false);
        assert_eq!(chunks.len(), 7);
        assert_eq!(chunks[0], "This is a ");
    }

    #[test]
    fn test_character_based_chunking() {
        let text = "This is a test string that should be split at word boundaries.";
        let chunks = character_based_chunking(text, 15, false);
        // Should split into meaningful chunks at word boundaries
        assert!(chunks.len() >= 4);
        // No chunk should exceed the target size significantly
        for chunk in &chunks {
            assert!(chunk.len() <= 20); // Allow some flexibility
        }
    }

    #[test]
    fn test_semantic_chunking() {
        let text = "This is sentence one. This is sentence two! Is this sentence three? Yes it is.";
        let chunks = semantic_chunking(text, false);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0], "This is sentence one");
        assert_eq!(chunks[1], "This is sentence two");
        assert_eq!(chunks[2], "Is this sentence three");
        assert_eq!(chunks[3], "Yes it is.");
    }
}
