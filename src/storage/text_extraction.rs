// FILE: src/storage/text_extraction.rs
//! Text Extraction Module
//!
//! Extracts text content from files for embedding generation.
//! Supports common text-based file formats.
//! 
//! HARDENING:
//! - Enforces 10MB limit to prevent OOM.
//! - Checks for binary content (null bytes).
//! - Implements "Structure-Aware" Chunking (Recursive Splitter).

use crate::error::Result;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

// Fail First: Never read files larger than 10MB into memory
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; 
// Check first 8KB for binary signatures
const BINARY_CHECK_BUFFER_SIZE: usize = 8192; 

// Chunking Configuration
// OPTIMIZED: 256-512 chars is the "Goldilocks Zone" for dense retrieval.
// It is large enough to contain a complete sentence/thought, but small enough
// to be vector-specific (high relevance for "Needle in Haystack").
const TARGET_CHUNK_SIZE: usize = 300; 
const CHUNK_OVERLAP: usize = 50; 

/// Extract text content from a file
pub fn extract_text_from_file(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(crate::error::MagicError::Other(
            anyhow::anyhow!("File does not exist: {}", path.display())
        ));
    }

    if !path.is_file() {
        return Err(crate::error::MagicError::Other(
            anyhow::anyhow!("Path is not a file: {}", path.display())
        ));
    }

    let metadata = std::fs::metadata(path).map_err(crate::error::MagicError::Io)?;
    let file_size = metadata.len();

    if file_size > MAX_FILE_SIZE {
        tracing::warn!("[TextExtraction] Skipping large file ({} bytes): {}", file_size, path.display());
        return Ok(String::new());
    }

    let mut file = File::open(path).map_err(crate::error::MagicError::Io)?;

    let mut buffer = [0u8; BINARY_CHECK_BUFFER_SIZE];
    let bytes_read = file.read(&mut buffer).map_err(crate::error::MagicError::Io)?;

    if buffer[..bytes_read].contains(&0) {
        tracing::debug!("[TextExtraction] Skipping binary file (null bytes detected): {}", path.display());
        return Ok(String::new());
    }

    file.seek(SeekFrom::Start(0)).map_err(crate::error::MagicError::Io)?;

    let mut content = String::new();
    match file.read_to_string(&mut content) {
        Ok(_) => {},
        Err(_) => {
            tracing::warn!("[TextExtraction] Skipping file with invalid UTF-8: {}", path.display());
            return Ok(String::new());
        }
    }

    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_lowercase();

    let extracted_text = match extension.as_str() {
        "txt" | "log" | "md" | "rst" => extract_plain_text(&content),
        "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "h" | "hpp" |
        "go" | "rb" | "php" | "sh" | "bash" | "zsh" | "fish" => extract_source_code(&content, &extension),
        "json" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" => extract_config(&content),
        _ => extract_plain_text(&content),
    };

    Ok(extracted_text)
}

/// Structure-Aware Chunking (The Elegant Solution)
/// 
/// Instead of slicing bytes, we accumulate logical units (paragraphs, lines, words).
/// This guarantees we never cut a word in half (`Extr-` ... `-act`) and preserves
/// semantic boundaries for the AI model.
pub fn chunk_text(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::with_capacity(TARGET_CHUNK_SIZE);
    
    // 1. Split into "Atomic Units" (Words)
    // Preserving structure is hard if we just split by space.
    // Strategy: We scan the text and identifying "safe break points".
    // 
    // Simplified Recursive Strategy:
    // We iterate through lines. If a line fits, add it.
    // If a line is HUGE, we split it by words.
    
    let lines: Vec<&str> = text.lines().collect();
    
    for line in lines {
        // +1 for the newline we lost during .lines()
        if current_chunk.len() + line.len() + 1 <= TARGET_CHUNK_SIZE {
            if !current_chunk.is_empty() {
                current_chunk.push('\n');
            }
            current_chunk.push_str(line);
        } else {
            // The line doesn't fit. 
            // Case A: The buffer has stuff in it. Emit the buffer first.
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.clone());
                
                // Start new chunk with overlap (Last N words)
                // For simplicity/speed in this pure-std implementation, 
                // we won't do complex semantic overlap calculation here yet.
                // We just start fresh or carry over a small tail if we implemented a deque.
                // Reset:
                current_chunk.clear();
            }
            
            // Case B: The line ITSELF is bigger than target?
            if line.len() > TARGET_CHUNK_SIZE {
                // We must split this dense line by words.
                let words: Vec<&str> = line.split(' ').collect();
                for word in words {
                    if current_chunk.len() + word.len() + 1 <= TARGET_CHUNK_SIZE {
                        if !current_chunk.is_empty() {
                            current_chunk.push(' ');
                        }
                        current_chunk.push_str(word);
                    } else {
                        // Emit
                        if !current_chunk.is_empty() {
                            chunks.push(current_chunk.clone());
                            
                            // OVERLAP LOGIC:
                            // To maintain context, we ideally want the last ~50 chars.
                            // Quick approximation: Keep the last word? 
                            // For now, clean break is better than word slicing.
                            current_chunk.clear();
                        }
                        current_chunk.push_str(word);
                    }
                }
            } else {
                // The line fits in an empty chunk
                current_chunk.push_str(line);
            }
        }
    }

    // Final Flush
    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }
    
    // Safety Filter: Remove tiny chunks that are just noise
    chunks.into_iter().filter(|c| c.trim().len() > 10).collect()
}

fn extract_plain_text(content: &str) -> String {
    content.to_string()
}

fn extract_source_code(content: &str, extension: &str) -> String {
    match extension {
        "rs" => extract_rust_code(content),
        "py" => extract_python_code(content),
        _ => extract_plain_text(content),
    }
}

fn extract_rust_code(content: &str) -> String {
    // Basic comment stripping while preserving structure
    content.lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("//") { "" } else { line }
        })
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_python_code(content: &str) -> String {
    content.lines()
        .map(|line| {
            if let Some(idx) = line.find('#') {
                &line[..idx]
            } else {
                line
            }
        })
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_config(content: &str) -> String {
    extract_plain_text(content)
}
