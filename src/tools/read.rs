use std::iter::once;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::{AgentStateChange, ToolResult};
use tokio::fs;

/// Struct to hold parsed arguments for the read tool
struct ReadArgs {
    offset: Option<usize>,
    limit: Option<usize>,
    paths: Vec<String>,
}

pub async fn execute_read(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    // Note: For read tool, we mainly use args, not body
    // Parse arguments
    let parsed_args = parse_arguments(args);

    // Handle empty paths case
    if parsed_args.paths.is_empty() {
        let error_msg = "No files specified for reading".to_string();

        if !silent_mode {
            // Use output buffer for error messages
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    // If offset or limit is specified, only read a single file
    if parsed_args.offset.is_some() || parsed_args.limit.is_some() {
        if parsed_args.paths.len() > 1 {
            let error_msg =
                "Offset and limit parameters can only be used with a single file".to_string();

            if !silent_mode {
                // Use buffer-based printing
                bprintln !(error:"{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
        return read_single_file(
            &parsed_args.paths[0],
            parsed_args.offset,
            parsed_args.limit,
            silent_mode,
        )
        .await;
    }

    // If there's only one path, use the single file/directory approach
    if parsed_args.paths.len() == 1 {
        return read_single_file(&parsed_args.paths[0], None, None, silent_mode).await;
    }

    // Multiple files case
    read_multiple_files(&parsed_args.paths, silent_mode).await
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

        if let Ok(val) = remaining_args[offset_start..offset_end]
            .trim()
            .parse::<usize>()
        {
            offset = Some(val);
        }

        // Remove the parameter from the string
        remaining_args = format!(
            "{} {}",
            &remaining_args[..offset_idx].trim(),
            &remaining_args[offset_end..].trim()
        )
        .trim()
        .to_string();
    }

    // Extract limit parameter
    if let Some(limit_idx) = remaining_args.find("limit=") {
        let limit_start = limit_idx + 6; // Length of "limit="
        let limit_end = find_param_end(&remaining_args[limit_start..])
            .map_or(remaining_args.len(), |end| limit_start + end);

        if let Ok(val) = remaining_args[limit_start..limit_end]
            .trim()
            .parse::<usize>()
        {
            limit = Some(val);
        }

        // Remove the parameter from the string
        remaining_args = format!(
            "{} {}",
            &remaining_args[..limit_idx].trim(),
            &remaining_args[limit_end..].trim()
        )
        .trim()
        .to_string();
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
async fn read_multiple_files(filepaths: &[String], silent_mode: bool) -> ToolResult {
    let mut agent_outputs = Vec::new();
    let mut all_successful = true;

    for filepath in filepaths {
        let result = read_file_content(filepath, None, None, silent_mode).await;
        agent_outputs.push(result.to_text());
        if !result.success {
            all_successful = false;
        }
    }

    let combined_agent_output = agent_outputs.join("\n\n");

    // Print combined output message if not in silent mode
    if !silent_mode {
        // Use buffer-based printing
        bprintln !(tool: "read",
            "{}📚 Read {} files:{}",
            FORMAT_BOLD,
            filepaths.len(),
            FORMAT_RESET
        );

        // Optionally, we could print more details about each file here
    }

    ToolResult {
        success: all_successful,
        state_change: AgentStateChange::Continue,
        content: vec![crate::llm::Content::Text {
            text: combined_agent_output,
        }],
    }
}

/// Helper function to read a single file or directory path
async fn read_single_file(
    filepath: &str,
    offset: Option<usize>,
    limit: Option<usize>,
    silent_mode: bool,
) -> ToolResult {
    // Validate path to prevent path traversal attacks
    let validated_path = match crate::tools::path_utils::validate_path(filepath) {
        Ok(path) => path,
        Err(e) => {
            let error_msg = format!("Security error: '{}': {}", filepath, e);

            if !silent_mode {
                // Use output buffer for error messages
                bprintln !(error:"{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
    };

    // Get the path as a reference
    let path = validated_path.as_path();

    // Check if path exists
    if !fs::try_exists(path).await.unwrap_or(false) {
        let error_msg = format!("Error: Path does not exist: '{}'", filepath);

        if !silent_mode {
            // Use output buffer for error messages
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    // Check if path is a directory
    if fs::metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        // Important: Use the validated path object, not the original string
        return read_directory(&validated_path.to_string_lossy(), silent_mode).await;
    }

    // Handle regular file - use the validated path, not the original string
    read_file_content(
        &validated_path.to_string_lossy(),
        offset,
        limit,
        silent_mode,
    )
    .await
}

/// Helper function to read file content with optional offset and limit
async fn read_file_content(
    filepath: &str,
    offset: Option<usize>,
    limit: Option<usize>,
    silent_mode: bool,
) -> ToolResult {
    // Validate file path to prevent path traversal attacks
    // (this validation may be redundant if called from read_single_file with validated path,
    // but we keep it for safety in case this function is called directly)
    let validated_path = match crate::tools::path_utils::validate_path(filepath) {
        Ok(path) => path,
        Err(e) => {
            let error_msg = format!("Security error for file '{}': {}", filepath, e);

            if !silent_mode {
                // Use output buffer for error messages
                bprintln !(error:"{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
    };

    match fs::read_to_string(&validated_path).await {
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

            // Get the display path from the validated path
            let safe_display_path = validated_path.to_string_lossy();

            // Format the output to clearly indicate line numbers
            let agent_output = format!(
                "File: {} (lines {}-{} of {}, {} lines read)\n\n```\n{}\n```",
                safe_display_path,
                start_line + 1,
                end_line,
                total_lines,
                lines_read,
                selected_lines
            );

            // Direct output to console if not in silent mode
            if !silent_mode {
                let num_lines = end_line - start_line;
                // Create a brief preview for console output
                let preview_lines = lines[start_line..end_line]
                    .iter()
                    .take(2)
                    .map(ToString::to_string)
                    .chain(once(format!(" + {} lines", end_line.saturating_sub(start_line).saturating_sub(2))))
                    .map(|line| format!("{}{}{}", FORMAT_GRAY, line, FORMAT_RESET))
                    .collect::<Vec<String>>()
                    .join("\n");
                
                // Use output buffer for read tool output
                if !preview_lines.is_empty() {
                    bprintln !(tool: "read",
                        "{}📄 Read: {} (lines {}-{} of {} total){}\n{}",
                        FORMAT_BOLD,
                        safe_display_path,
                        start_line + 1,
                        end_line,
                        total_lines,
                        FORMAT_RESET,
                        preview_lines
                    );
                } else {
                    bprintln !(tool: "read",
                        "{}📄 Read: {} (lines {}-{} of {} total){}",
                        FORMAT_BOLD,
                        safe_display_path,
                        start_line + 1,
                        end_line,
                        total_lines,
                        FORMAT_RESET
                    );
                }
            }

            ToolResult::success(agent_output)
        }
        Err(e) => {
            let error_msg = format!("Error reading file '{}': {}", filepath, e);

            if !silent_mode {
                // Use buffer-based printing
                bprintln !(error:"{}", error_msg);
            }

            ToolResult::error(error_msg)
        }
    }
}

/// Helper function to list directory contents
async fn read_directory(dirpath: &str, silent_mode: bool) -> ToolResult {
    // Validate directory path to prevent path traversal attacks
    // (this validation may be redundant if called from read_single_file with validated path,
    // but we keep it for safety in case this function is called directly)
    let validated_path = match crate::tools::path_utils::validate_directory(dirpath) {
        Ok(path) => path,
        Err(e) => {
            let error_msg = format!("Security error for directory '{}': {}", dirpath, e);

            if !silent_mode {
                // Use output buffer for error messages
                bprintln !(error:"{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
    };

    // Get the display path from the validated path
    let safe_display_path = validated_path.to_string_lossy();

    match fs::read_dir(&validated_path).await {
        Ok(mut entries) => {
            let mut files = Vec::new();
            let mut dirs = Vec::new();

            // Collect directory entries
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    if let Ok(filename) = entry.file_name().into_string() {
                        if file_type.is_dir() {
                            dirs.push(format!("{}/", filename));
                        } else {
                            files.push(filename);
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

            // Format output for agent, using safe display path
            let agent_output = format!(
                "Directory: {} ({} entries)\n\n{}",
                safe_display_path, entry_count, content
            );

            // Direct output to console if not in silent mode
            if !silent_mode {
                // Build directory output string directly
                let mut output = format!(
                    "{}📁 Directory: {} ({} items){}\n",
                    FORMAT_BOLD, safe_display_path, entry_count, FORMAT_RESET
                );

                // Add directories with trailing slash and bold formatting
                for dir in &dirs {
                    let dir_name = dir.trim_end_matches('/');
                    output.push_str(&format!(
                        "{}{}{}/{}\n",
                        FORMAT_BOLD, FORMAT_GRAY, dir_name, FORMAT_RESET
                    ));
                }

                // Add files
                for file in &files {
                    output.push_str(&format!("{}{}{}\n", FORMAT_GRAY, file, FORMAT_RESET));
                }

                // Use buffer-based printing
                bprintln !(tool: "read", "{}", output.trim_end());
            }

            ToolResult {
                success: true,
                state_change: Default::default(),
                content: vec![crate::llm::Content::Text { text: agent_output }],
            }
        }
        Err(e) => {
            let error_msg = format!("Error reading directory '{}': {}", safe_display_path, e);

            if !silent_mode {
                // Use buffer-based printing
                bprintln !(error:"{}", error_msg);
            }

            ToolResult::error(error_msg)
        }
    }
}
