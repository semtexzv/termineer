use crate::constants::{
    FORMAT_BOLD, FORMAT_DIFF_ADDED, FORMAT_DIFF_ADDED_CHAR, FORMAT_DIFF_DELETED, 
    FORMAT_DIFF_DELETED_CHAR, FORMAT_DIFF_SECTION, FORMAT_RESET, PATCH_DELIMITER_AFTER,
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
            let error_msg = format!("Security error for file '{}': {}", filename, e);

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
                // Use buffer-based printing directly
                bprintln !(error:"Error reading file '{}': {}", filename, e);
            }

            return ToolResult::error(format!("Error reading file '{}': {}", filename, e));
        }
    };

    // Parse the patch content
    let before_delimiter = match patch_content.find(PATCH_DELIMITER_BEFORE) {
        Some(pos) => pos,
        None => {
            if !silent_mode {
                // Use buffer-based printing directly
                bprintln !(error:"Missing '{}' delimiter in patch", PATCH_DELIMITER_BEFORE);
            }

            return ToolResult::error(format!(
                "Missing '{}' delimiter in patch",
                PATCH_DELIMITER_BEFORE
            ));
        }
    };

    let after_delimiter = match patch_content[before_delimiter..].find(PATCH_DELIMITER_AFTER) {
        Some(pos) => before_delimiter + pos,
        None => {
            if !silent_mode {
                // Use buffer-based printing directly
                bprintln !(error:"Missing '{}' delimiter in patch", PATCH_DELIMITER_AFTER);
            }

            return ToolResult::error(format!(
                "Missing '{}' delimiter in patch",
                PATCH_DELIMITER_AFTER
            ));
        }
    };

    let end_delimiter = match patch_content[after_delimiter..].find(PATCH_DELIMITER_END) {
        Some(pos) => after_delimiter + pos,
        None => {
            if !silent_mode {
                // Use buffer-based printing directly
                bprintln !(error:"Missing '{}' delimiter in patch", PATCH_DELIMITER_END);
            }

            return ToolResult::error(format!(
                "Missing '{}' delimiter in patch",
                PATCH_DELIMITER_END
            ));
        }
    };

    // Check the order of delimiters
    if before_delimiter >= after_delimiter {
        let error_msg =
            "Invalid patch: BEFORE delimiter must come before AFTER delimiter".to_string();

        if !silent_mode {
            // Use buffer-based printing
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    if after_delimiter >= end_delimiter {
        let error_msg = "Invalid patch: AFTER delimiter must come before END delimiter".to_string();

        if !silent_mode {
            // Use buffer-based printing
            bprintln !(error:"{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

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
            bprintln !(error:"{} {:?}", error_msg, patch_content);
        }

        return ToolResult::error(error_msg);
    }

    let before_text = patch_content[before_start..after_delimiter].trim();
    let after_text = patch_content[after_start..end_delimiter].trim();

    // Apply the patch
    if !file_content.contains(before_text) {
        if !silent_mode {
            // Use buffer-based printing directly
            bprintln !(error:"Text to replace not found in the file: '{}'", before_text);
        }

        return ToolResult::error(format!(
            "Text to replace not found in the file: '{}'",
            before_text
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
                "Successfully patched file '{}' at lines {}-{} (replaced {} lines with {} lines)",
                safe_display_path,
                start_line_number,
                end_line_number,
                before_text_lines,
                after_text.lines().count()
            );

            // Create an enhanced unified diff with character-level changes
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
                
                // Track line numbers for visualization
                let mut line_num_before = start_line_number;
                let mut line_num_after = start_line_number;

                // Function to add line numbers to unified diff
                let format_line_num = |line: &str, before_num: usize, after_num: Option<usize>| {
                    if let Some(after) = after_num {
                        format!("{:4} {:4} │ {}", before_num, after, line)
                    } else {
                        format!("{:4}      │ {}", before_num, line)
                    }
                };

                // Add header with column descriptions
                unified_diff.push(format!("{}OLD  NEW  │ CONTENT{}", FORMAT_BOLD, FORMAT_RESET));
                unified_diff.push(format!("{}─────────┼─────────────────────────────────{}", FORMAT_DIFF_SECTION, FORMAT_RESET));

                while i < before_lines.len() || j < after_lines.len() {
                    if i < before_lines.len()
                        && j < after_lines.len()
                        && before_lines[i] == after_lines[j]
                        && lcs.contains(&(i, j))
                    {
                        // Line is unchanged (with line numbers now)
                        unchanged_buffer.push(format_line_num(
                            before_lines[i], 
                            line_num_before, 
                            Some(line_num_after)
                        ));

                        // If we're not already showing unchanged lines and buffer is too large, trim it
                        if !showing_unchanged && unchanged_buffer.len() > context_lines * 2 {
                            // Add separator with distinct formatting
                            if i > context_lines {
                                unified_diff.push(format!("{}...{}", FORMAT_DIFF_SECTION, FORMAT_RESET));
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
                        line_num_before += 1;
                        line_num_after += 1;
                    } else {
                        // We're in a changed section, so show any buffered unchanged lines
                        if !unchanged_buffer.is_empty() {
                            unified_diff.extend(unchanged_buffer.drain(..));
                        }
                        showing_unchanged = false;

                        // Enhanced line-level diff with line numbers and better highlighting
                        // Collect lines to be deleted and added for possible char-level diff
                        let orig_i = i;
                        let orig_j = j;
                        let orig_line_num_before = line_num_before;
                        let orig_line_num_after = line_num_after;
                        
                        // Find all consecutive deleted lines
                        let mut deleted_lines = Vec::new();
                        while i < before_lines.len() && 
                              (j >= after_lines.len() || !lcs.contains(&(i, j))) {
                            deleted_lines.push((i, before_lines[i], line_num_before));
                            i += 1;
                            line_num_before += 1;
                        }
                        
                        // Find all consecutive added lines
                        let mut added_lines = Vec::new();
                        while j < after_lines.len() && 
                              (orig_i >= before_lines.len() || !lcs.contains(&(orig_i, j))) {
                            added_lines.push((j, after_lines[j], line_num_after));
                            j += 1;
                            line_num_after += 1;
                        }
                        
                        // If we have exactly one line deleted and one added, try char-level diff
                        if deleted_lines.len() == 1 && added_lines.len() == 1 {
                            let (_, del_text, del_num) = deleted_lines[0];
                            let (_, add_text, add_num) = added_lines[0];
                            
                            // Add a visual separator for the change section
                            unified_diff.push(format!("{}┄┄┄┄┄┄┄┄┄┼┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄{}", 
                                FORMAT_DIFF_SECTION, FORMAT_RESET));
                            
                            // Display the deleted line with proper formatting
                            unified_diff.push(format!("{}{}{}", FORMAT_DIFF_DELETED, 
                                format_line_num(del_text, del_num, None), FORMAT_RESET));
                                
                            // Display the added line with proper formatting
                            unified_diff.push(format!("{}{}{}", FORMAT_DIFF_ADDED,
                                format_line_num(add_text, 0, Some(add_num)), FORMAT_RESET));
                                
                            // Add character-level diff if the strings are similar enough
                            if del_text.len() > 0 && add_text.len() > 0 && 
                               (del_text.len() as f32 / add_text.len() as f32).abs() < 2.0 {
                                
                                // Find character differences
                                let mut char_diff = String::new();
                                let del_chars: Vec<char> = del_text.chars().collect();
                                let add_chars: Vec<char> = add_text.chars().collect();
                                
                                // Simple character diff algorithm
                                let mut i = 0;
                                let mut j = 0;
                                let max_i = del_chars.len();
                                let max_j = add_chars.len();
                                
                                // Visual diff line with proper spacing for alignment
                                char_diff.push_str("      │ ");
                                
                                while i < max_i || j < max_j {
                                    if i < max_i && j < max_j && del_chars[i] == add_chars[j] {
                                        // Matching character
                                        char_diff.push(del_chars[i]);
                                        i += 1;
                                        j += 1;
                                    } else {
                                        // Start of difference
                                        let mut del_str = String::new();
                                        let mut add_str = String::new();
                                        
                                        // Collect deleted chars until next match
                                        while i < max_i && (j >= max_j || del_chars[i] != add_chars[j]) {
                                            del_str.push(del_chars[i]);
                                            i += 1;
                                        }
                                        
                                        // Collect added chars until next match
                                        let orig_i = i;
                                        while j < max_j && (orig_i >= max_i || del_chars[orig_i] != add_chars[j]) {
                                            add_str.push(add_chars[j]);
                                            j += 1;
                                        }
                                        
                                        // Highlight changes
                                        if !del_str.is_empty() {
                                            char_diff.push_str(&format!("{}{}{}", 
                                                FORMAT_DIFF_DELETED_CHAR, del_str, FORMAT_RESET));
                                        }
                                        
                                        if !add_str.is_empty() {
                                            char_diff.push_str(&format!("{}{}{}", 
                                                FORMAT_DIFF_ADDED_CHAR, add_str, FORMAT_RESET));
                                        }
                                    }
                                }
                                
                                // Add character diff to output
                                unified_diff.push(format!("{}Character diff:{} {}", 
                                    FORMAT_BOLD, FORMAT_RESET, char_diff));
                            }
                            
                            // Add closing separator
                            unified_diff.push(format!("{}┄┄┄┄┄┄┄┄┄┼┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄{}", 
                                FORMAT_DIFF_SECTION, FORMAT_RESET));
                        } else {
                            // Standard multi-line diff
                            // Show deleted lines
                            for (_, text, num) in deleted_lines {
                                unified_diff.push(format!("{}{}{}", FORMAT_DIFF_DELETED, 
                                    format_line_num(text, num, None), FORMAT_RESET));
                            }
                            
                            // Show added lines
                            for (_, text, num) in added_lines {
                                unified_diff.push(format!("{}{}{}", FORMAT_DIFF_ADDED,
                                    format_line_num("", 0, Some(num)), FORMAT_RESET) + text);
                            }
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
                        unified_diff.push(format!("{}...{}", FORMAT_DIFF_SECTION, FORMAT_RESET));
                        // Take just the last 'to_show' lines
                        let trailing_context: Vec<String> =
                            unchanged_buffer.drain(buffer_len - to_show..).collect();
                        unified_diff.extend(trailing_context);
                    } else {
                        // Show all the lines in the buffer
                        unified_diff.extend(unchanged_buffer.drain(..));
                    }
                }

                // Create an enhanced header for the diff summary
                let diff_header = format!(
                    "{}🔄 PATCH SUMMARY: {}{}", 
                    FORMAT_BOLD, safe_display_path, FORMAT_RESET
                );

                // Add statistics to the diff header
                let stats_info = format!(
                    "{}Stats: {} lines removed, {} lines added, {} lines modified{}",
                    FORMAT_BOLD,
                    removed_lines,
                    added_lines,
                    if removed_lines == added_lines { removed_lines } else { 0 },
                    FORMAT_RESET
                );

                // Add line information to the diff header
                let line_info = format!(
                    "{}@@ Lines {}-{} modified (file has {} lines total) @@{}",
                    FORMAT_BOLD,
                    start_line_number,
                    end_line_number,
                    file_content.lines().count(),
                    FORMAT_RESET
                );

                // Combine all diff lines into a string
                let full_diff = unified_diff.join("\n");

                // Use buffer-based printing with enhanced formatting
                bprintln !(tool: "patch", "{}\n{}\n{}\n\n{}", 
                    diff_header, 
                    stats_info,
                    line_info, 
                    full_diff);
            }

            ToolResult::success(agent_output)
        }
        Err(e) => {
            if !silent_mode {
                // Use buffer-based printing directly
                bprintln !(error:"Error writing patched file '{}': {}", filename, e);
            }

            ToolResult::error(format!("Error writing patched file '{}': {}", filename, e))
        }
    }
}

// Helper function to compute the Longest Common Subsequence with improved algorithm
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

    // Track which lines are part of the LCS
    let mut lcs_positions = vec![vec![false; n]; m];
    
    // Use a better backtracking algorithm to find exact matched lines
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            lcs_positions[i - 1][j - 1] = true;
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    
    // Convert to the expected format
    let mut lcs = Vec::new();
    for i in 0..m {
        for j in 0..n {
            if lcs_positions[i][j] {
                lcs.push((i, j));
            }
        }
    }
    
    // Sort to ensure correct order
    lcs.sort_by_key(|&(i, j)| (i, j));
    lcs
}

// No character-level diffing needed for line-level diff
