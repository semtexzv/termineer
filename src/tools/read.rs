use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET, FORMAT_YELLOW};
use crate::llm::{Content, ImageSource};
use crate::tools::{AgentStateChange, ToolResult};
use image::GenericImageView;
use std::iter::once;
use tokio::fs; // Import the required trait

/// Maximum number of lines the read tool can read at once.
/// This prevents loading extremely large files into the conversation
/// which could overwhelm token limits or make the UI unresponsive.
/// When this limit is reached, truncation notices are shown with
/// instructions on how to access additional content.
const MAX_READABLE_LINES: usize = 1000;

/// Struct to hold parsed arguments for the read tool
struct ReadArgs {
    offset: Option<usize>,
    limit: Option<usize>,
    lines_specified: bool, // Flag to indicate if lines=START-END was used
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

    // If offset, limit, or lines is specified, only read a single file
    if parsed_args.offset.is_some() || parsed_args.limit.is_some() || parsed_args.lines_specified {
        if parsed_args.paths.len() > 1 {
            let error_msg =
                "Offset, limit, and lines parameters can only be used with a single file".to_string();

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

/// Parse the command arguments into a structured format, handling parameters anywhere.
fn parse_arguments(args: &str) -> ReadArgs {
    let mut offset: Option<usize> = None;
    let mut limit: Option<usize> = None;
    let mut lines_specified = false;
    let mut paths = Vec::new();

    let parts: Vec<&str> = args.trim().split_whitespace().collect();

    for part in parts {
        if part.starts_with("offset=") {
            if let Some(val_str) = part.strip_prefix("offset=") {
                if let Ok(val) = val_str.parse::<usize>() {
                    offset = Some(val);
                } else {
                    bprintln!(warn: "Invalid offset value: {}", val_str);
                }
            }
        } else if part.starts_with("limit=") {
            if let Some(val_str) = part.strip_prefix("limit=") {
                if let Ok(val) = val_str.parse::<usize>() {
                    limit = Some(val);
                } else {
                    bprintln!(warn: "Invalid limit value: {}", val_str);
                }
            }
        } else if part.starts_with("lines=") {
            if let Some(range_str) = part.strip_prefix("lines=") {
                let range_parts: Vec<&str> = range_str.splitn(2, '-').collect();
                if range_parts.len() == 2 {
                    if let (Ok(start), Ok(end)) = (range_parts[0].parse::<usize>(), range_parts[1].parse::<usize>()) {
                        if start >= 1 && end >= start {
                            // Convert 1-based lines to 0-based offset and limit
                            offset = Some(start - 1);
                            limit = Some(end - start + 1);
                            lines_specified = true;
                        } else {
                            bprintln!(warn: "Invalid line range in lines={}: start must be >= 1 and end >= start", range_str);
                        }
                    } else {
                        bprintln!(warn: "Invalid number format in lines={}", range_str);
                    }
                } else {
                    bprintln!(warn: "Invalid format for lines parameter. Use lines=START-END. Got: {}", range_str);
                }
            }
        } else {
            // Assume it's a path
            paths.push(part.to_string());
        }
    }

    // If lines= was specified, it overrides offset= and limit=
    // We already set offset and limit based on lines= if it was valid.
    // No need for explicit override logic here, as the last parsed value wins.
    // However, if lines_specified is true, we should probably clear offset/limit if they were set *before* lines=
    // Let's refine: parse lines= first, then offset/limit, but ignore offset/limit if lines_specified is true.

    // Re-parse to prioritize lines=
    let mut final_offset: Option<usize> = None;
    let mut final_limit: Option<usize> = None;
    let mut final_lines_specified = false;
    let mut final_paths = Vec::new();

    for part in args.trim().split_whitespace() {
        if part.starts_with("lines=") {
            if let Some(range_str) = part.strip_prefix("lines=") {
                let range_parts: Vec<&str> = range_str.splitn(2, '-').collect();
                if range_parts.len() == 2 {
                    if let (Ok(start), Ok(end)) = (range_parts[0].parse::<usize>(), range_parts[1].parse::<usize>()) {
                        if start >= 1 && end >= start {
                            final_offset = Some(start - 1);
                            final_limit = Some(end - start + 1);
                            final_lines_specified = true;
                        } else {
                             bprintln!(warn: "Invalid line range in lines={}: start must be >= 1 and end >= start. Ignoring.", range_str);
                        }
                    } else {
                         bprintln!(warn: "Invalid number format in lines={}. Ignoring.", range_str);
                    }
                } else {
                     bprintln!(warn: "Invalid format for lines parameter. Use lines=START-END. Got: {}. Ignoring.", range_str);
                }
            }
        } else if part.starts_with("offset=") {
            if !final_lines_specified { // Only parse if lines= wasn't specified or was invalid
                if let Some(val_str) = part.strip_prefix("offset=") {
                    if let Ok(val) = val_str.parse::<usize>() {
                        final_offset = Some(val);
                    } else {
                        bprintln!(warn: "Invalid offset value: {}. Ignoring.", val_str);
                    }
                }
            }
        } else if part.starts_with("limit=") {
             if !final_lines_specified { // Only parse if lines= wasn't specified or was invalid
                if let Some(val_str) = part.strip_prefix("limit=") {
                    if let Ok(val) = val_str.parse::<usize>() {
                        final_limit = Some(val);
                    } else {
                        bprintln!(warn: "Invalid limit value: {}. Ignoring.", val_str);
                    }
                }
            }
        } else {
            // Assume it's a path
            final_paths.push(part.to_string());
        }
    }


    ReadArgs {
        offset: final_offset,
        limit: final_limit,
        lines_specified: final_lines_specified,
        paths: final_paths,
    }
}

// Removed find_param_end as it's no longer needed with the new parsing logic

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

    // Get the display path from the validated path
    let safe_display_path = validated_path.to_string_lossy();

    // Check if this is an image file by extension
    let extension = validated_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase());

    // Only detect image formats that we can actually process based on the crate features
    let is_image = match extension.as_deref() {
        Some("jpg") | Some("jpeg") | Some("png") => true,
        _ => false,
    };

    // Handle image files differently
    if is_image {
        return read_image_file(&validated_path, safe_display_path.to_string(), silent_mode).await;
    }

    // Regular text file handling
    match fs::read_to_string(&validated_path).await {
        Ok(content) => {
            // Split content into lines
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            // Apply offset and limit
            let start_line = offset.unwrap_or(0).min(total_lines);

            // Determine the requested end line based on the limit parameter or full file
            let requested_end_line = match limit {
                Some(l) => (start_line + l).min(total_lines),
                None => total_lines,
            };

            // Check if we need to truncate due to MAX_READABLE_LINES
            let was_truncated = (requested_end_line - start_line) > MAX_READABLE_LINES;

            // Apply the maximum line limit
            let end_line = if was_truncated {
                start_line + MAX_READABLE_LINES
            } else {
                requested_end_line
            };

            // Extract the requested lines
            let selected_lines = lines[start_line..end_line].join("\n");
            let lines_read = end_line - start_line;

            // Format the output to clearly indicate line numbers and truncation if it occurred
            let truncation_notice = if was_truncated {
                // Suggest the next offset using 1-based line numbers for user clarity
                let next_offset_suggestion = start_line + MAX_READABLE_LINES; // 0-based offset for the next chunk
                format!("\n\n{} TRUNCATION NOTICE {}\nFile content was truncated to {} lines maximum.\nTo read additional content, use offset parameter:\n  read offset={} limit=1000 {}\nOr use lines=START-END:\n  read lines={}-{} {}\n", 
                    "=".repeat(15), 
                    "=".repeat(15),
                    MAX_READABLE_LINES,
                    next_offset_suggestion, // Use 0-based offset for the parameter
                    safe_display_path,
                    next_offset_suggestion + 1, // Start line (1-based)
                    next_offset_suggestion + 1 + MAX_READABLE_LINES.saturating_sub(1), // End line (1-based)
                    safe_display_path
                )
            } else {
                String::new()
            };

            let agent_output = format!(
                "File: {} (lines {}-{} of {}, {} lines read{})\n\n{}\n{}",
                safe_display_path,
                start_line + 1,
                end_line,
                total_lines,
                lines_read,
                if was_truncated { ", truncated" } else { "" },
                selected_lines,
                truncation_notice
            );

            // Direct output to console if not in silent mode
            if !silent_mode {
                // Create a brief preview for console output
                let preview_lines = lines[start_line..end_line]
                    .iter()
                    .take(2)
                    .map(ToString::to_string)
                    .chain(once(format!(
                        "+ {} lines",
                        end_line.saturating_sub(start_line).saturating_sub(2)
                    )))
                    .map(|line| format!("{}{}{}", FORMAT_GRAY, line, FORMAT_RESET))
                    .collect::<Vec<String>>()
                    .join("\n");

                // Use output buffer for read tool output
                if !preview_lines.is_empty() {
                    let truncated_mark = if was_truncated {
                        format!(" {}{}{}", FORMAT_YELLOW, "⚠️ TRUNCATED", FORMAT_RESET)
                    } else {
                        String::new()
                    };

                    bprintln !(tool: "read",
                        "{}📄 Read: {} (lines {}-{} of {} total){}{}{}{}",
                        FORMAT_BOLD,
                        safe_display_path,
                        start_line + 1,
                        end_line,
                        total_lines,
                        truncated_mark,
                        FORMAT_RESET,
                        "\n",
                        preview_lines
                    );
                } else {
                    let truncated_mark = if was_truncated {
                        format!(" {}{}{}", FORMAT_YELLOW, "⚠️ TRUNCATED", FORMAT_RESET)
                    } else {
                        String::new()
                    };

                    bprintln !(tool: "read",
                        "{}📄 Read: {} (lines {}-{} of {} total){}{}",
                        FORMAT_BOLD,
                        safe_display_path,
                        start_line + 1,
                        end_line,
                        total_lines,
                        truncated_mark,
                        FORMAT_RESET
                    );
                }

                // Add detailed truncation notice to console output if needed
                if was_truncated {
                    bprintln !(tool: "read",
                        "{}⚠️  File too large: content truncated to {} lines maximum.{}",
                        FORMAT_YELLOW,
                        MAX_READABLE_LINES,
                        FORMAT_RESET
                    );
                    // Suggest both offset and lines syntax for the next chunk
                    let next_offset_suggestion = start_line + MAX_READABLE_LINES; // 0-based offset
                    bprintln !(tool: "read",
                        "{}   To read more, use: read offset={} limit=1000 {}{}",
                        FORMAT_YELLOW,
                        next_offset_suggestion,
                        safe_display_path,
                        FORMAT_RESET
                    );
                    bprintln !(tool: "read",
                        "{}   Or use: read lines={}-{} {}{}",
                        FORMAT_YELLOW,
                        next_offset_suggestion + 1, // Start line (1-based)
                        next_offset_suggestion + 1 + MAX_READABLE_LINES.saturating_sub(1), // End line (1-based)
                        safe_display_path,
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

/// Special handler for image files
async fn read_image_file(
    validated_path: &std::path::Path,
    safe_display_path: String,
    silent_mode: bool,
) -> ToolResult {
    use base64::{engine::general_purpose, Engine as _};
    use image::{ImageFormat};
    use std::io::Cursor;

    // Read the file as binary
    let file_bytes = match fs::read(validated_path).await {
        Ok(bytes) => bytes,
        Err(e) => {
            let error_msg = format!("Error reading image file '{}': {}", safe_display_path, e);
            if !silent_mode {
                bprintln!(error:"{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    };

    // Check file size limit (1MB)
    const MAX_IMAGE_SIZE: usize = 1_048_576; // 1MB in bytes
    if file_bytes.len() > MAX_IMAGE_SIZE {
        let error_msg = format!(
            "Image file '{}' is too large ({} MB). Maximum size is 1MB. Try using a smaller image.",
            safe_display_path,
            file_bytes.len() / 1_048_576
        );
        if !silent_mode {
            bprintln!(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Try to determine format and load the image
    let img_format = match validated_path.extension().and_then(|ext| ext.to_str()) {
        Some("jpg") | Some("jpeg") => Some(ImageFormat::Jpeg),
        Some("png") => Some(ImageFormat::Png),
        // Note: Only supporting formats included in Cargo.toml features
        _ => None,
    };

    // Get media type and determine if we support this format
    let media_type = match validated_path.extension().and_then(|ext| ext.to_str()) {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        _ => {
            // If we don't recognize the format, return an error
            let error_msg = format!(
                "Unsupported image format for file '{}'. Currently only JPEG and PNG formats are supported.",
                safe_display_path
            );
            if !silent_mode {
                bprintln!(error:"{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    };

    // For dimensions display
    let mut width = 0;
    let mut height = 0;

    // Process image if needed (resize large images)
    let processed_bytes = if let Ok(img) = image::load_from_memory(&file_bytes) {
        // Get dimensions
        let (w, h) = img.dimensions();
        width = w;
        height = h;

        // Resize if the image is too large (similar to the screenshot tool)
        if width > 1600 || height > 1200 {
            let scale_factor = f32::min(1600.0 / width as f32, 1200.0 / height as f32);
            let new_width = (width as f32 * scale_factor) as u32;
            let new_height = (height as f32 * scale_factor) as u32;

            if !silent_mode {
                bprintln!(tool: "read",
                    "Resizing image from {}x{} to {}x{}",
                    width, height, new_width, new_height
                );
            }

            // Store the new dimensions
            width = new_width;
            height = new_height;

            let resized = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);

            // Save to a buffer
            let mut output = Vec::new();
            let mut cursor = Cursor::new(&mut output);

            if let Some(format) = img_format {
                if resized.write_to(&mut cursor, format).is_ok() {
                    output
                } else {
                    // If can't resize/convert, use original
                    file_bytes.clone()
                }
            } else {
                // Default to JPEG if format unknown
                if resized.write_to(&mut cursor, ImageFormat::Jpeg).is_ok() {
                    output
                } else {
                    file_bytes.clone()
                }
            }
        } else {
            // Use original bytes if no resizing needed
            file_bytes.clone()
        }
    } else {
        // Not a valid image or couldn't be processed, use original bytes
        file_bytes.clone()
    };

    // Encode to base64
    let base64_data = general_purpose::STANDARD.encode(&processed_bytes);
    let file_size = processed_bytes.len();

    // Create agent output with image details
    let agent_output = format!(
        "Image: {} ({}x{}, {} bytes)",
        safe_display_path, width, height, file_size
    );

    // Log output for UI
    if !silent_mode {
        bprintln!(tool: "read",
            "{}🖼️ Read image: {} ({}x{}, {} KB){}",
            FORMAT_BOLD,
            safe_display_path,
            width,
            height,
            file_size / 1024,
            FORMAT_RESET
        );
    }

    // Return success with both text and image content
    ToolResult {
        success: true,
        state_change: AgentStateChange::Continue,
        content: vec![
            Content::Text { text: agent_output },
            Content::Image {
                source: ImageSource::Base64 {
                    media_type: media_type.to_string(),
                    data: base64_data,
                },
            },
        ],
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
