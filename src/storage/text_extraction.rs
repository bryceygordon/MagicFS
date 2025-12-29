//! Text Extraction Module
//!
//! Extracts text content from files for embedding generation.
//! Supports common text-based file formats.
//! 
//! HARDENING:
//! - Enforces 10MB limit to prevent OOM.
//! - Checks for binary content (null bytes) to prevent garbage indexing.

use crate::error::Result;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

// Fail First: Never read files larger than 10MB into memory
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; 
// Check first 8KB for binary signatures
const BINARY_CHECK_BUFFER_SIZE: usize = 8192; 

/// Extract text content from a file
/// Supports: .txt, .rs, .md, .json, .yaml, .toml, .toml, and other text files
pub fn extract_text_from_file(path: &Path) -> Result<String> {
    // Check if file exists and is readable
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

    // 1. FAIL FIRST: Check File Size
    let metadata = std::fs::metadata(path).map_err(|e| crate::error::MagicError::Io(e))?;
    let file_size = metadata.len();

    if file_size > MAX_FILE_SIZE {
        tracing::warn!("[TextExtraction] Skipping large file ({} bytes): {}", file_size, path.display());
        // Return empty string to signal "nothing to index", but don't error out the pipeline
        return Ok(String::new());
    }

    let mut file = File::open(path)
        .map_err(|e| crate::error::MagicError::Io(e))?;

    // 2. FAIL FIRST: Check for Binary Content (Null Bytes)
    // Read the beginning of the file to check for \0
    let mut buffer = [0u8; BINARY_CHECK_BUFFER_SIZE];
    let bytes_read = file.read(&mut buffer).map_err(|e| crate::error::MagicError::Io(e))?;

    if buffer[..bytes_read].contains(&0) {
        tracing::debug!("[TextExtraction] Skipping binary file (null bytes detected): {}", path.display());
        return Ok(String::new());
    }

    // Rewind to start after check
    file.seek(SeekFrom::Start(0)).map_err(|e| crate::error::MagicError::Io(e))?;

    // 3. Read Content (Safe now)
    let mut content = String::new();
    // We use read_to_string which does UTF-8 validation
    match file.read_to_string(&mut content) {
        Ok(_) => {},
        Err(_) => {
            tracing::warn!("[TextExtraction] Skipping file with invalid UTF-8: {}", path.display());
            return Ok(String::new());
        }
    }

    // Extract relevant text based on file extension
    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_lowercase();

    let extracted_text = match extension.as_str() {
        // Plain text files
        "txt" | "log" | "md" | "rst" => extract_plain_text(&content),

        // Source code files
        "rs" | "py" | "js" | "ts" | "java" | "c" | "cpp" | "h" | "hpp" |
        "go" | "rb" | "php" | "sh" | "bash" | "zsh" | "fish" => extract_source_code(&content, &extension),

        // Config files
        "json" | "yaml" | "yml" | "toml" | "ini" | "cfg" | "conf" => extract_config(&content),

        // Other text files
        _ => extract_plain_text(&content),
    };

    Ok(extracted_text)
}

/// Extract plain text content
fn extract_plain_text(content: &str) -> String {
    // Remove excessive whitespace and normalize
    content.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract source code content (removes comments where possible)
fn extract_source_code(content: &str, extension: &str) -> String {
    match extension {
        "rs" => extract_rust_code(content),
        "py" => extract_python_code(content),
        _ => extract_plain_text(content),
    }
}

/// Extract Rust source code (removes comments)
fn extract_rust_code(content: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut in_block_comment = false;

    for &line in &lines {
        let line = line.trim();

        if line.starts_with("/*") {
            in_block_comment = true;
            if line.ends_with("*/") && line.len() > 4 {
                // Single-line block comment
                in_block_comment = false;
            }
            continue;
        }

        if in_block_comment {
            if line.ends_with("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if line.starts_with("//") {
            continue;
        }

        if !line.is_empty() {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// Extract Python source code (removes comments)
fn extract_python_code(content: &str) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = content.lines().collect();

    for &line in &lines {
        let line = line.trim();

        // Remove inline comments
        let line_no_comments = if let Some(hash_pos) = line.find('#') {
            &line[..hash_pos]
        } else {
            line
        };

        if !line_no_comments.trim().is_empty() {
            result.push_str(line_no_comments);
            result.push('\n');
        }
    }

    result
}

/// Extract configuration files (extracts keys and values)
fn extract_config(content: &str) -> String {
    // For now, treat config as plain text
    // Could be enhanced to parse JSON, YAML, etc.
    extract_plain_text(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_extract_plain_text() {
        let content = "Line 1\n\nLine 2\n  Line 3  \n";
        let result = extract_plain_text(content);
        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_binary_detection() {
        // Create a temporary binary file
        let path = Path::new("/tmp/magicfs_test_binary.bin");
        let mut file = File::create(&path).unwrap();
        // Write null bytes
        file.write_all(b"Hello\0World").unwrap();
        
        let result = extract_text_from_file(path).unwrap();
        assert_eq!(result, "", "Binary file should return empty string");
        
        // Cleanup
        let _ = std::fs::remove_file(path);
    }
}
