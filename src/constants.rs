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

// Patch tool delimiters
pub const PATCH_DELIMITER_BEFORE: &str = "<<<<BEFORE";
pub const PATCH_DELIMITER_AFTER: &str = "<<<<AFTER";
pub const PATCH_DELIMITER_END: &str = "<<<<END";

// Templates for help and usage
pub const HELP_TEMPLATE: &str = r#"
# autoswe Help

## Available Commands
  /help                  - Display this help
  /clear                 - Clear conversation history
  /system TEXT           - Set a custom system prompt
  /model NAME            - Change the model (e.g., claude-3-opus-20240229)
  /tools on|off          - Enable or disable tools
  /thinking NUMBER       - Set thinking budget in tokens
  /stats                 - Show detailed token usage statistics
  
## Session Management
  /session list          - List all saved sessions in current directory
  /session all           - List sessions from all directories
  /session save NAME     - Save current session with a name
  /session load NAME/ID  - Load a session by name or ID
  /session delete NAME/ID- Delete a session by name or ID
  /session resume        - Resume the last active session
  /exit                  - Exit the program

## Effective Interaction
  • Be specific about what you want the assistant to accomplish
  • Provide context about your project when relevant
  • The assistant will automatically continue working until task completion
  • For complex tasks, break them down into smaller steps
  • After task completion, you can start a new task

## Available Tools (For the Assistant's Use)
  • Shell: Execute commands and get real-time output
  • Read: Examine file contents with optional line limits
  • Write: Create or overwrite files with new content
  • Patch: Make targeted changes to specific parts of files
  • Done: Signal task completion with a summary

These tools are exclusively for the assistant's use - you don't need to use them directly.
Just describe what you want to accomplish in natural language, and the assistant will 
use the appropriate tools as needed.

## Example Requests
  • "Create a React component for a login form"
  • "Debug why this Python script is giving IndexError"
  • "Analyze this codebase and suggest improvements"
  • "Update this config to enable CORS support"
  • "Create a unit test for this function"
"#;

pub const USAGE_TEMPLATE: &str = r#"
# autoswe: AI-powered Software Engineering Assistant

## Usage
  autoswe [OPTIONS] [QUERY]

  • If QUERY is provided, runs in non-interactive mode
  • If QUERY is not provided, starts an interactive console session

## Options
  --model MODEL_NAME     Specify the AI model to use
                         The provider is automatically inferred from model prefixes:
                         - Anthropic: claude-3-opus, claude-3-sonnet, etc.
                         - OpenAI: gpt-4, o1, etc. (coming soon)
                         
                         You can also explicitly specify the provider with:
                         --model anthropic/claude-3-opus or --model openai/gpt-4
                         
                         (default: claude-3-7-sonnet-20250219)
  --system PROMPT        Provide a custom system prompt
  --stop-sequences SEQ   Comma-separated list of stopping sequences
                         (e.g., "Human:,Assistant:")
  --no-tools             Disable tool usage (enabled by default)
  --help                 Display this help message

## Token Optimization Options
  --max-history NUMBER   Set maximum history length (default: 100)
  --thinking-budget NUM  Set thinking budget in tokens (default: 4096)
  --minimal-prompt       Use a condensed system prompt to save tokens

## Session Management
  --resume               Automatically resume the last session

## Environment Setup
  ANTHROPIC_API_KEY      Your Anthropic API key (required)
                         Can be set in .env file or environment variables

## Examples
  autoswe "Analyze this Node.js project and suggest optimizations"
  autoswe --model claude-3-opus-20240229 "Fix bugs in the login component"
  autoswe --no-tools "Explain how React's virtual DOM works"

## Capabilities
The assistant can help with:
  • Code analysis and development
  • Debugging and troubleshooting
  • Refactoring and optimization
  • Documentation and explanation
  • Design patterns and architecture

## Tool Integration
The assistant seamlessly uses these tools to assist you:
  • Shell: Execute commands to explore and modify your environment
  • Read: Examine file contents to understand your codebase
  • Write: Create new files or overwrite existing ones
  • Patch: Make targeted changes to specific parts of files
  • Done: Signal task completion with a comprehensive summary

These tools are exclusively for the assistant's use - simply describe what you
want to accomplish, and the assistant will leverage the appropriate tools to
complete your request efficiently.
"#;

// System prompts are now defined in the prompts module

// Make the format_template function public so it can be called from prompts
pub fn format_template(template: &str) -> String {
    template
        .replace("{TOOL_START}", TOOL_START)
        .replace("{TOOL_END}", TOOL_END)
        .replace("{TOOL_RESULT_START}", TOOL_RESULT_START)
        .replace("{TOOL_RESULT_END}", TOOL_RESULT_END)
        .replace("{TOOL_ERROR_START}", TOOL_ERROR_START)
        .replace("{TOOL_ERROR_END}", TOOL_ERROR_END)
        .replace("{PATCH_DELIMITER_BEFORE}", PATCH_DELIMITER_BEFORE)
        .replace("{PATCH_DELIMITER_AFTER}", PATCH_DELIMITER_AFTER)
        .replace("{PATCH_DELIMITER_END}", PATCH_DELIMITER_END)
}
