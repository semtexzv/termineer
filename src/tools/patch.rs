use crate::constants::{
    FORMAT_BOLD, FORMAT_DIFF_ADDED, FORMAT_DIFF_DELETED, FORMAT_RESET, PATCH_DELIMITER_AFTER,
    PATCH_DELIMITER_BEFORE, PATCH_DELIMITER_END,
};
use crate::tools::ToolResult;
use tokio::fs;

pub async fn execute_patch(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Extract filename from args
    let filename = args.trim();

    if filename.is_empty() {
        let error_msg = "Patch tool requires a filename as an argument".to_string();

        if !silent_mode {
            // Use buffer-based printing
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    if body.trim().is_empty() {
        let error_msg = "Patch tool requires patch content in the body".to_string();

        if !silent_mode {
            // Use buffer-based printing
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    // Use body as the patch content
    let patch_content = body;

    // Validate path to prevent path traversal attacks
    let validated_path = match crate::tools::path_utils::validate_path(filename) {
        Ok(path) => path,
        Err(e) => {
            let error_msg = format!("Security error for file '{filename}': {e}");

            if !silent_mode {
                // Use buffer-based printing directly
                bprintln !(error:"{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
    };

    // Read the file content
    let file_content = match fs::read_to_string(&validated_path).await {
        Ok(content) => content,
        Err(e) => {
            if !silent_mode {
                bprintln !(error:"Error reading file '{filename}': {e}");
            }

            return ToolResult::error(format!("Error reading file '{filename}': {e}"));
        }
    };

    // Parse the patch content
    let before_delimiter = match patch_content.find(PATCH_DELIMITER_BEFORE) {
        Some(pos) => pos,
        None => {
            if !silent_mode {
                bprintln !(error:"Missing '{PATCH_DELIMITER_BEFORE}' delimiter in patch");
            }

            return ToolResult::error(format!(
                "Missing '{PATCH_DELIMITER_BEFORE}' delimiter in patch"
            ));
        }
    };

    bprintln!(dev: "rest: {}", &patch_content[before_delimiter..]);

    let after_delimiter = match patch_content[before_delimiter..].find(PATCH_DELIMITER_AFTER) {
        Some(pos) => before_delimiter + pos,
        None => {
            if !silent_mode {
                bprintln !(error:"Missing '{PATCH_DELIMITER_AFTER}' delimiter in patch");
            }

            return ToolResult::error(format!(
                "Missing '{PATCH_DELIMITER_AFTER}' delimiter in patch"
            ));
        }
    };

    let end_delimiter = match patch_content[after_delimiter..].find(PATCH_DELIMITER_END) {
        Some(pos) => after_delimiter + pos,
        None => {
            if !silent_mode {
                bprintln !(error:"Missing '{PATCH_DELIMITER_END}' delimiter in patch");
            }

            return ToolResult::error(format!(
                "Missing '{PATCH_DELIMITER_END}' delimiter in patch"
            ));
        }
    };

    // Extract the before and after text
    // Skip the delimiter line itself by finding the next newline
    let before_start =
        match patch_content[before_delimiter + PATCH_DELIMITER_BEFORE.len()..].find('\n') {
            Some(pos) => before_delimiter + PATCH_DELIMITER_BEFORE.len() + pos + 1,
            None => before_delimiter + PATCH_DELIMITER_BEFORE.len(),
        };

    let after_start =
        match patch_content[after_delimiter + PATCH_DELIMITER_AFTER.len()..].find('\n') {
            Some(pos) => after_delimiter + PATCH_DELIMITER_AFTER.len() + pos + 1,
            None => after_delimiter + PATCH_DELIMITER_AFTER.len(),
        };

    // Ensure indices are in bounds
    if before_start >= after_delimiter || after_start >= end_delimiter {
        let error_msg = "Invalid patch format: delimiter positions are invalid".to_string();

        if !silent_mode {
            // Use buffer-based printing
            bprintln !(error:"{error_msg} {patch_content:?}");
        }

        return ToolResult::error(error_msg);
    }

    let before_text = patch_content[before_start..after_delimiter].trim();
    let after_text = patch_content[after_start..end_delimiter].trim();

    // Count occurrences of the before_text in file_content
    let mut count = 0;
    let mut start_index = 0;
    while let Some(index) = file_content[start_index..].find(before_text) {
        count += 1;
        start_index += index + 1;
        
        // Early exit if we've already found multiple occurrences
        if count > 1 {
            break;
        }
    }

    if count == 0 {
        if !silent_mode {
            // Use buffer-based printing directly
            bprintln !(error:"Text to replace not found in the file: '{before_text}'");
        }

        return ToolResult::error(format!(
            "Text to replace not found in the file: '{before_text}'"
        ));
    }

    // Check if the text appears multiple times in the file
    if count > 1 {
        if !silent_mode {
            // Use buffer-based printing directly
            bprintln !(error:"Patch failed: Text to replace occurs multiple times ({count} occurrences) in the file. Please provide more context to make the patch unique.");
        }

        return ToolResult::error(format!(
            "Ambiguous patch: Text to replace occurs multiple times ({count} occurrences) in the file. Please provide more context to make the patch unique."
        ));
    }

    let new_content = file_content.replace(before_text, after_text);

    // Get a safe display path for output messages
    let safe_display_path = validated_path.to_string_lossy();

    // Write the updated content (using validated path)
    match fs::write(&validated_path, new_content).await {
        Ok(_) => {
            // Detailed output for the agent with line number information
            // First, find the line numbers in the original file where the patch was applied
            let before_text_lines = before_text.lines().count();
            let start_line_number = if let Some(pos) = file_content.find(before_text) {
                // Count newlines to determine line number
                file_content[..pos].lines().count() + 1 // +1 because line numbers are 1-indexed
            } else {
                0 // Should never happen since we already checked if before_text exists
            };

            let end_line_number = start_line_number + before_text_lines - 1;

            let agent_output = format!(
                "Successfully patched file '{safe_display_path}' at lines {start_line_number}-{end_line_number} (replaced {before_text_lines} lines with {} lines)",
                after_text.lines().count()
            );

            // Create a sophisticated unified diff
            let before_lines: Vec<&str> = before_text.lines().collect();
            let after_lines: Vec<&str> = after_text.lines().collect();

            // Count lines changed
            let removed_lines = before_text.lines().count();
            let added_lines = after_text.lines().count();

            // Only generate and print the diff if not in silent mode
            if !silent_mode {
                // Compute the longest common subsequence (LCS) using dynamic programming
                let lcs = longest_common_subsequence(&before_lines, &after_lines);

                // Generate the diff using the LCS
                let mut unified_diff = Vec::new();
                let mut i = 0;
                let mut j = 0;

                // Context lines to show before and after changes
                let context_lines = 2;
                let mut showing_unchanged = false;
                let mut unchanged_buffer = Vec::new();

                while i < before_lines.len() || j < after_lines.len() {
                    if i < before_lines.len()
                        && j < after_lines.len()
                        && before_lines[i] == after_lines[j]
                        && lcs.contains(&(i, j))
                    {
                        // Line is unchanged
                        let line = before_lines[i];
                        unchanged_buffer.push(format!("  {line}"));

                        // If we're not already showing unchanged lines and buffer is too large, trim it
                        if !showing_unchanged && unchanged_buffer.len() > context_lines * 2 {
                            // Add separator if we skipped lines
                            if i > context_lines {
                                unified_diff.push("  ...".to_string());
                            }

                            // Keep only the last few context lines
                            let buffer_len = unchanged_buffer.len();
                            let new_buffer: Vec<String> = unchanged_buffer
                                .drain(buffer_len - context_lines..)
                                .collect();
                            unchanged_buffer = new_buffer;
                        }

                        i += 1;
                        j += 1;
                    } else {
                        // We're in a changed section, so show any buffered unchanged lines
                        if !unchanged_buffer.is_empty() {
                            unified_diff.extend(unchanged_buffer.drain(..));
                        }
                        showing_unchanged = false;

                        // Check if we need to delete a line from 'before'
                        if j >= after_lines.len()
                            || (i < before_lines.len() && !lcs.contains(&(i, j)))
                        {
                            unified_diff.push(format!(
                                "{FORMAT_DIFF_DELETED}- {}{FORMAT_RESET}",
                                before_lines[i]
                            ));
                            i += 1;
                        }
                        // Check if we need to add a line from 'after'
                        else if i >= before_lines.len()
                            || (j < after_lines.len() && !lcs.contains(&(i, j)))
                        {
                            unified_diff.push(format!(
                                "{FORMAT_DIFF_ADDED}+ {}{FORMAT_RESET}",
                                after_lines[j]
                            ));
                            j += 1;
                        }
                    }

                    // If we've processed a batch of changes, mark that we're showing unchanged lines again
                    if !unchanged_buffer.is_empty() && unchanged_buffer.len() >= context_lines {
                        showing_unchanged = true;
                    }
                }

                // Add any remaining unchanged lines
                if !unchanged_buffer.is_empty() {
                    // Only show a limited number of trailing context lines
                    let buffer_len = unchanged_buffer.len();
                    let to_show = buffer_len.min(context_lines);

                    if to_show < buffer_len {
                        unified_diff.push("  ...".to_string());
                        // Take just the last 'to_show' lines
                        let trailing_context: Vec<String> =
                            unchanged_buffer.drain(buffer_len - to_show..).collect();
                        unified_diff.extend(trailing_context);
                    } else {
                        // Show all the lines in the buffer
                        unified_diff.extend(unchanged_buffer.drain(..));
                    }
                }

                // Create a header for the diff summary
                let diff_header = format!(
                    "{FORMAT_BOLD}🔄 Patch: {safe_display_path} (-{removed_lines} lines, +{added_lines} lines){FORMAT_RESET}"
                );

                // Add line information to the diff header
                let line_info = format!(
                    "{FORMAT_BOLD}@@ Lines {start_line_number}-{end_line_number} modified (file has {} lines total) @@{FORMAT_RESET}",
                    file_content.lines().count()
                );

                // Combine all diff lines into a string
                let full_diff = unified_diff.join("\n");

                // Use buffer-based printing
                bprintln !(tool: "patch", "{}\n{}\n\n{}", diff_header, line_info, full_diff);
            }

            ToolResult::success(agent_output)
        }
        Err(e) => {
            if !silent_mode {
                // Use buffer-based printing directly
                bprintln !(error:"Error writing patched file '{filename}': {e}");
            }

            ToolResult::error(format!("Error writing patched file '{filename}': {e}"))
        }
    }
}

// Helper function to compute the Longest Common Subsequence
fn longest_common_subsequence<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<(usize, usize)> {
    let m = a.len();
    let n = b.len();

    // Create a matrix to store lengths of LCS
    let mut dp = vec![vec![0; n + 1]; m + 1];

    // Fill the dp table
    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Reconstruct the LCS
    let mut lcs = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            lcs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] > dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    lcs.reverse();
    lcs
}
