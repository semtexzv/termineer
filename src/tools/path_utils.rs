//! Path safety utilities to prevent path traversal attacks
//!
//! This module provides functions to validate file paths and prevent
//! path traversal attacks that could otherwise expose sensitive files.

use std::env;
use std::io;
use std::path::{Path, PathBuf};

/// Checks if a path is safe to access by ensuring it doesn't escape the current directory
/// or access sensitive system locations
///
/// # Arguments
/// * `path` - The path to validate
///
/// # Returns
/// * `Ok(canonicalized_path)` - If the path is safe, returns the canonicalized path
/// * `Err(error)` - If the path is unsafe or there's an error processing it
pub fn validate_path(path: &str) -> io::Result<PathBuf> {
    // Get the current working directory as the base directory
    let base_dir = env::current_dir()?;

    // Create a path from the input
    let target_path = Path::new(path);

    // Canonicalize both paths to resolve "..", symlinks, etc.
    // Note: canonicalize requires the path to exist, so we need to handle non-existent paths differently
    let canonical_base = base_dir.canonicalize()?;

    // For paths that don't exist yet (e.g., for write operations), we need to check the parent directory
    let target_canonical = if target_path.exists() {
        target_path.canonicalize()?
    } else if let Some(parent) = target_path.parent() {
        if parent.exists() {
            // Canonicalize the parent and then append the filename
            let mut parent_canonical = parent.canonicalize()?;
            if let Some(file_name) = target_path.file_name() {
                parent_canonical.push(file_name);
                parent_canonical
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid path: no filename component",
                ));
            }
        } else {
            // If parent doesn't exist, we can't safely validate the path
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Parent directory does not exist",
            ));
        }
    } else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid path"));
    };

    // Convert to string representation for easier comparison
    let base_str = canonical_base.to_string_lossy();
    let target_str = target_canonical.to_string_lossy();

    // Check if target path starts with the base path (is within working directory)
    if target_str.starts_with(&*base_str) {
        // Path is within the allowed directory
        Ok(target_canonical)
    } else {
        // Path is outside allowed directory - potential path traversal attack
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "Access denied: path is outside the working directory: {}",
                path
            ),
        ))
    }
}

/// Checks if a path is safe for directory operations
///
/// This is a variant of validate_path specifically for directories
pub fn validate_directory(path: &str) -> io::Result<PathBuf> {
    validate_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_safe_path() {
        // Create a temporary directory and file for testing
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test-file.txt");
        
        // Create the test file
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "Test content").unwrap();
        
        // Change to the temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();
        
        // Test with a file we know exists in our current directory
        let result = validate_path("test-file.txt");
        
        // Restore the original working directory
        env::set_current_dir(original_dir).unwrap();
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_traversal_attempt() {
        let result = validate_path("../../../etc/passwd");
        assert!(result.is_err());
    }
}
