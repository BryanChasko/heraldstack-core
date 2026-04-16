// ingest_utils.rs
// Shared error handling and utility functions for ingest binaries

/// Validate each line of a JSONL file, returning (valid, invalid) counts
pub fn validate_jsonl_lines(content: &str) -> (usize, usize) {
    let mut valid = 0;
    let mut invalid = 0;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(_) => valid += 1,
            Err(_) => invalid += 1,
        }
    }
    (valid, invalid)
}
