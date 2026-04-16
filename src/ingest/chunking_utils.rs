// chunking_utils.rs
// Shared utilities for text chunking in ingestion pipelines

/// Chunk a text field for embedding, using character-based chunking if needed.
/// Returns a Vec of chunk strings, each <= max_len chars.
pub fn chunk_field(text: &str, max_len: usize) -> Vec<String> {
    let text_len = text.chars().count();
    let safe_len = usize::min(max_len, 250); // Guarantee <=250 chars per chunk
    let chunks = if text_len > safe_len {
        let mut chunks = Vec::new();
        let mut start = 0;
        let chars: Vec<char> = text.chars().collect();
        while start < text_len {
            let end = usize::min(start + safe_len, text_len);
            let chunk: String = chars[start..end].iter().collect();
            chunks.push(chunk);
            start = end;
        }
        chunks
    } else {
        vec![text.to_string()]
    };
    // Debug print: chunk count and sizes
    println!(
        "[chunk_field] {} chars input, {} chunks:",
        text_len,
        chunks.len()
    );
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  Chunk {}: {} chars", i + 1, chunk.chars().count());
    }
    chunks
}

/// Chunk all relevant fields in a character/entity JSON object for embedding.
/// Returns a Vec of (field_name, chunk_text) pairs.
pub fn chunk_entity_fields(obj: &serde_json::Value, max_len: usize) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    if let Some(name) = obj.get("character_name").and_then(|v| v.as_str()) {
        for chunk in chunk_field(name, max_len) {
            fields.push(("character_name".to_string(), chunk));
        }
    }
    if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
        for chunk in chunk_field(desc, max_len) {
            fields.push(("description".to_string(), chunk));
        }
    }
    if let Some(aff) = obj.get("affiliations").and_then(|v| v.as_array()) {
        let joined = aff
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        for chunk in chunk_field(&joined, max_len) {
            fields.push(("affiliations".to_string(), chunk));
        }
    }
    if let Some(attrs) = obj.get("core_attributes").and_then(|v| v.as_array()) {
        let joined = attrs
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        for chunk in chunk_field(&joined, max_len) {
            fields.push(("core_attributes".to_string(), chunk));
        }
    }
    if let Some(themes) = obj.get("inspirational_themes").and_then(|v| v.as_array()) {
        let joined = themes
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        for chunk in chunk_field(&joined, max_len) {
            fields.push(("inspirational_themes".to_string(), chunk));
        }
    }
    if let Some(traits) = obj.get("traits").and_then(|v| v.as_array()) {
        let joined = traits
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        for chunk in chunk_field(&joined, max_len) {
            fields.push(("traits".to_string(), chunk));
        }
    }
    fields
}
