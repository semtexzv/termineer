// These color constants are kept for future UI enhancements
// and to maintain a consistent color scheme across the application
#![allow(dead_code)]

// Tool delimiters
pub const TOOL_START: &str = "<tool>";
pub const TOOL_END: &str = "</tool>";
pub const TOOL_RESULT_START_PREFIX: &str = "<tool_result";
pub const TOOL_RESULT_START: &str = "<tool_result>";
pub const TOOL_RESULT_END: &str = "</tool_result>";
pub const TOOL_ERROR_START_PREFIX: &str = "<tool_error";
pub const TOOL_ERROR_START: &str = "<tool_error>";
pub const TOOL_ERROR_END: &str = "</tool_error>";

pub const MD_TOOL_CALL_START: &str = "```tool_use ";
pub const MD_TOOL_RESULT_START: &str = "```result [";
pub const MD_TOOL_ERROR_START: &str = "```error [";
pub const MD_CODE_END: &str = "```";


pub const FORMAT_RESET: &str = "\x1b[0m";
pub const FORMAT_BOLD: &str = "\x1b[1m";
pub const FORMAT_GRAY: &str = "\x1b[90m";
pub const FORMAT_RED: &str = "\x1b[31m";
pub const FORMAT_GREEN: &str = "\x1b[32m";
pub const FORMAT_YELLOW: &str = "\x1b[33m";
pub const FORMAT_BLUE: &str = "\x1b[34m";
pub const FORMAT_MAGENTA: &str = "\x1b[35m";
pub const FORMAT_CYAN: &str = "\x1b[36m";
pub const FORMAT_RED_BG: &str = "\x1b[41m";
pub const FORMAT_GREEN_BG: &str = "\x1b[42m";
// Colors for diff output (using 256-color indexed colors)
pub const FORMAT_DIFF_DELETED: &str = "\x1b[48;5;224m";     // Light pastel red background
pub const FORMAT_DIFF_ADDED: &str = "\x1b[48;5;193m";       // Light pastel green background
pub const FORMAT_DIFF_DELETED_CHAR: &str = "\x1b[48;5;217m"; // Darker red for char-level changes
pub const FORMAT_DIFF_ADDED_CHAR: &str = "\x1b[48;5;157m";   // Darker green for char-level changes
pub const FORMAT_DIFF_SECTION: &str = "\x1b[38;5;39m";      // Bright blue for section markers

// Patch tool delimiters
pub const PATCH_DELIMITER_BEFORE: &str = "<<<<BEFORE";
pub const PATCH_DELIMITER_AFTER: &str = "<<<<AFTER";
pub const PATCH_DELIMITER_END: &str = "<<<<END";

// Constants for controlling tool output truncation
// Maximum length for tool outputs before truncation is applied
pub const MAX_TOOL_OUTPUT_LENGTH: usize = 100_000; // 100KB default limit
                                                   // Text to insert when content is truncated
pub const TRUNCATION_PLACEHOLDER: &str = "\n[...CONTENT TRUNCATED...]\n";
// Whether to preserve content from the end of truncated output
pub const PRESERVE_OUTPUT_END: bool = true;
// If preserving end content, how many characters to keep from the end
pub const PRESERVED_END_LENGTH: usize = 2000;
// How many characters to keep from the beginning
pub const PRESERVED_START_LENGTH: usize = 4000;
