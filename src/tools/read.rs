use std::fs;
use std::path::Path;
use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};

/// Struct to hold parsed arguments for the read tool
struct ReadArgs {
    offset: Option<usize>,
    limit: Option<usize>,
    paths: Vec<String>,
}

pub fn execute_read(args: &str, _body: &str) -> ToolResult {
    // Note: For read tool, we mainly use args, not body
    // Parse arguments
    let parsed_args = parse_arguments(args);
    
    // Handle empty paths case
    if parsed_args.paths.is_empty() {
        let error_msg = "No files specified for reading".to_string();
        return ToolResult {
            success: false,
            user_output: error_msg.clone(),
            agent_output: error_msg,
        };
    }
    
    // If offset or limit is specified, only read a single file
    if parsed_args.offset.is_some() || parsed_args.limit.is_some() {
        if parsed_args.paths.len() > 1 {
            let error_msg = "Offset and limit parameters can only be used with a single file".to_string();
            return ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            };
        }
        return read_single_file(&parsed_args.paths[0], parsed_args.offset, parsed_args.limit);
    }
    
    // If there's only one path, use the single file/directory approach
    if parsed_args.paths.len() == 1 {
        return read_single_file(&parsed_args.paths[0], None, None);
    }
    
    // Multiple files case
    read_multiple_files(&parsed_args.paths)
}

/// Parse the command arguments into a structured format
fn parse_arguments(args: &str) -> ReadArgs {
    let mut offset: Option<usize> = None;
    let mut limit: Option<usize> = None;
    let mut remaining_args = args.trim().to_string();
    
    // Extract offset parameter
    if let Some(offset_idx) = remaining_args.find("offset=") {
        let offset_start = offset_idx + 7; // Length of "offset="
        let offset_end = find_param_end(&remaining_args[offset_start..])
            .map_or(remaining_args.len(), |end| offset_start + end);
            
        if let Ok(val) = remaining_args[offset_start..offset_end].trim().parse::<usize>() {
            offset = Some(val);
        }
        
        // Remove the parameter from the string
        remaining_args = format!(
            "{} {}", 
            &remaining_args[..offset_idx].trim(), 
            &remaining_args[offset_end..].trim()
        ).trim().to_string();
    }
    
    // Extract limit parameter
    if let Some(limit_idx) = remaining_args.find("limit=") {
        let limit_start = limit_idx + 6; // Length of "limit="
        let limit_end = find_param_end(&remaining_args[limit_start..])
            .map_or(remaining_args.len(), |end| limit_start + end);
            
        if let Ok(val) = remaining_args[limit_start..limit_end].trim().parse::<usize>() {
            limit = Some(val);
        }
        
        // Remove the parameter from the string
        remaining_args = format!(
            "{} {}", 
            &remaining_args[..limit_idx].trim(), 
            &remaining_args[limit_end..].trim()
        ).trim().to_string();
    }
    
    // Split remaining arguments into paths
    let paths: Vec<String> = remaining_args
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    
    ReadArgs {
        offset,
        limit,
        paths,
    }
}

/// Helper function to find the end of a parameter value
fn find_param_end(s: &str) -> Option<usize> {
    s.find(|c: char| c.is_whitespace())
}

/// Read multiple files and combine their outputs
fn read_multiple_files(filepaths: &[String]) -> ToolResult {
    let mut agent_outputs = Vec::new();
    let mut user_outputs = Vec::new();
    let mut all_successful = true;
    
    for filepath in filepaths {
        let result = read_file_content(filepath, None, None);
        if result.success {
            agent_outputs.push(result.agent_output);
            user_outputs.push(result.user_output);
        } else {
            agent_outputs.push(result.agent_output.clone());
            user_outputs.push(result.agent_output); // Use error message directly
            all_successful = false;
        }
    }
    
    let combined_agent_output = agent_outputs.join("\n\n");
    let combined_user_output = format!(
        "{}📚 Read {} files:{} {}", 
        FORMAT_BOLD,
        filepaths.len(),
        FORMAT_RESET,
        user_outputs.join(" | ")
    );
    
    ToolResult {
        success: all_successful,
        user_output: combined_user_output,
        agent_output: combined_agent_output,
    }
}

/// Helper function to read a single file or directory path
fn read_single_file(filepath: &str, offset: Option<usize>, limit: Option<usize>) -> ToolResult {
    let path = Path::new(filepath);
    
    // Check if path exists
    if !path.exists() {
        let error_msg = format!("Error: Path does not exist: '{}'", filepath);
        return ToolResult {
            success: false,
            user_output: error_msg.clone(),
            agent_output: error_msg,
        };
    }
    
    // Check if path is a directory
    if path.is_dir() {
        return read_directory(filepath);
    }
    
    // Handle regular file
    read_file_content(filepath, offset, limit)
}

/// Helper function to read file content with optional offset and limit
fn read_file_content(filepath: &str, offset: Option<usize>, limit: Option<usize>) -> ToolResult {
    match fs::read_to_string(filepath) {
        Ok(content) => {
            // Split content into lines
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();
            
            // Apply offset and limit
            let start_line = offset.unwrap_or(0).min(total_lines);
            let end_line = match limit {
                Some(l) => (start_line + l).min(total_lines),
                None => total_lines,
            };
            
            // Extract the requested lines
            let selected_lines = lines[start_line..end_line].join("\n");
            let lines_read = end_line - start_line;
            
            // Format the output to clearly indicate line numbers
            let agent_output = format!(
                "File: {} (lines {}-{} of {}, {} lines read)\n\n```\n{}\n```",
                filepath, 
                start_line+1, 
                end_line, 
                total_lines, 
                lines_read,
                selected_lines
            );
            
            // Create a brief preview for user output
            let preview_lines = lines[start_line..end_line].iter()
                .take(2)
                .cloned()
                .collect::<Vec<&str>>()
                .join("\n");
                
            let preview = if !preview_lines.is_empty() {
                format!("\n{}{}{}", FORMAT_GRAY, preview_lines, FORMAT_RESET)
            } else {
                "".to_string()
            };
            
            let user_output = format!(
                "{}📄 Read: {} (lines {}-{} of {} total){}{}",
                FORMAT_BOLD,
                filepath, 
                start_line+1, 
                end_line, 
                total_lines,
                FORMAT_RESET,
                preview
            );
            
            ToolResult {
                success: true,
                user_output,
                agent_output,
            }
        },
        Err(e) => {
            let error_msg = format!("Error reading file '{}': {}", filepath, e);
            ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            }
        },
    }
}

/// Helper function to list directory contents
fn read_directory(dirpath: &str) -> ToolResult {
    match fs::read_dir(dirpath) {
        Ok(entries) => {
            let mut files = Vec::new();
            let mut dirs = Vec::new();
            
            // Collect directory entries
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(file_type) = entry.file_type() {
                        if let Ok(filename) = entry.file_name().into_string() {
                            if file_type.is_dir() {
                                dirs.push(format!("{}/", filename));
                            } else {
                                files.push(filename);
                            }
                        }
                    }
                }
            }
            
            // Sort entries alphabetically
            dirs.sort();
            files.sort();
            
            // Combine directories and files
            let all_entries = [&dirs[..], &files[..]].concat();
            let entry_count = all_entries.len();
            let content = all_entries.join("\n");
            
            // Format output for agent
            let agent_output = format!(
                "Directory: {} ({} entries)\n\n{}",
                dirpath,
                entry_count,
                content
            );
            
            // Format output for user display
            let mut list_output = Vec::new();
            list_output.push(format!("Directory: {} ({} items)", dirpath, entry_count));
            
            // Add directories with trailing slash and bold formatting
            for dir in &dirs {
                let dir_name = dir.trim_end_matches('/');
                list_output.push(format!("{}{}/{}", FORMAT_BOLD, dir_name, FORMAT_RESET));
            }
            
            // Add files
            for file in &files {
                list_output.push(file.clone());
            }
            
            let user_output = format!(
                "{}📁 {}{}\n{}",
                FORMAT_BOLD,
                list_output[0],
                FORMAT_RESET,
                list_output[1..].join("\n")
            );
            
            ToolResult {
                success: true,
                user_output,
                agent_output,
            }
        },
        Err(e) => {
            let error_msg = format!("Error reading directory '{}': {}", dirpath, e);
            ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            }
        },
    }
}