//! Text Extraction Module
//!
//! Extracts text content from files for embedding generation.
//! Supports common text-based file formats.

use crate::error::Result;
use std::fs::File;
use std::io::Read;
use std::path::Path;

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

    // Read file content
    let mut file = File::open(path)
        .map_err(|e| crate::error::MagicError::Io(e))?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| crate::error::MagicError::Io(e))?;

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

    #[test]
    fn test_extract_plain_text() {
        let content = "Line 1\n\nLine 2\n  Line 3  \n";
        let result = extract_plain_text(content);
        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_extract_rust_code() {
        let content = r#"// This is a comment
fn main() {
    // Inline comment
    let x = 5; /* Block comment */
    /* Multi-line
       block comment */
    println!("Hello");
}
"#;
        let result = extract_rust_code(content);
        assert!(!result.contains("This is a comment"));
        assert!(result.contains("fn main()"));
        assert!(result.contains("let x = 5;"));
    }

    #[test]
    fn test_extract_python_code() {
        let content = r#"# This is a comment
def hello():
    x = 5  # inline comment
    return "hello"
"#;
        let result = extract_python_code(content);
        assert!(!result.contains("This is a comment"));
        assert!(result.contains("def hello():"));
        assert!(result.contains("x = 5"));
    }

    #[test]
    fn test_extract_config() {
        let content = r#"{
    "key1": "value1",
    "key2": "value2"
}"#;
        let result = extract_config(content);
        assert!(result.contains("key1"));
        assert!(result.contains("value1"));
    }
}