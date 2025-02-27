// Tool delimiters
pub const TOOL_START: &str = "<tool>";
pub const TOOL_END: &str = "</tool>";
pub const TOOL_RESULT_START: &str = "<tool_result>";
pub const TOOL_RESULT_END: &str = "</tool_result>";
pub const TOOL_ERROR_START: &str = "<tool_error>";
pub const TOOL_ERROR_END: &str = "</tool_error>";

// Patch tool delimiters
pub const PATCH_DELIMITER_BEFORE: &str = "<<<<BEFORE";
pub const PATCH_DELIMITER_AFTER: &str = "<<<<AFTER";
pub const PATCH_DELIMITER_END: &str = "<<<<";

// Templates for help and usage
pub const HELP_TEMPLATE: &str = r#"
Available Commands:
  /help         - Display this help
  /clear        - Clear conversation history
  /system TEXT  - Set a system prompt
  /model NAME   - Change the model (e.g., claude-3-opus-20240229)
  /tools on|off - Enable or disable tools
  /exit         - Exit the program

Interaction:
  - Claude will automatically continue working on your task until completion
  - Once a task is complete, you can enter a new query

Claude has access to system tools that allow it to:
  - Execute shell commands
  - Read and write files
  - Make targeted updates to files
  - Signal task completion

These tools are exclusively for Claude's use - you don't need to use them directly.
Just describe what you want to accomplish in natural language, and Claude will 
use the appropriate tools to complete your request.
"#;

pub const USAGE_TEMPLATE: &str = r#"
Usage: AutoSWE [OPTIONS] [QUERY]

If QUERY is provided, runs in non-interactive mode and outputs only the response.
If QUERY is not provided, starts an interactive console session.

Options:
  --model MODEL_NAME     Specify the Claude model to use
                         (default: claude-3-7-sonnet-20250219)
  --system PROMPT        Provide a system prompt for Claude
  --no-tools             Disable tool usage (enabled by default)
  --help                 Display this help message

Environment Variables:
  ANTHROPIC_API_KEY      Your Anthropic API key (required)

Example:
  AutoSWE --model claude-3-haiku-20240307 "What is the capital of France?"

Interaction:
- Claude will automatically continue working on your task until completion
- Once a task is complete, you can enter a new query

Claude has access to system tools that allow it to:
- Execute shell commands
- Read and write files
- Make targeted updates to files
- Signal task completion

These tools are exclusively for Claude's use - you don't need to use them directly.
Just describe what you want to accomplish in natural language, and Claude will
use the appropriate tools to complete your request.
"#;

pub const SYSTEM_PROMPT_TEMPLATE: &str = r#"You are Claude, an AI assistant by Anthropic. You are connected to a custom console interface with tool support.

## Available Tools

### Shell
Execute shell commands on the user's system:
{TOOL_START}shell [command]{TOOL_END}

Example:
{TOOL_START}shell ls -la{TOOL_END}

### Read
Read the contents of a file:
{TOOL_START}read [offset=N] [limit=M] [filepath]{TOOL_END}
- The `offset` parameter (optional) specifies the starting line number (0-indexed)
- The `limit` parameter (optional) specifies the maximum number of lines to read
- For large files, only the first and last few lines will be shown

Examples:
{TOOL_START}read /etc/hosts{TOOL_END}
{TOOL_START}read offset=10 limit=20 /etc/hosts{TOOL_END}

### Write
Write content to a file:
{TOOL_START}write [filepath]
[content on multiple lines]
{TOOL_END}

Example:
{TOOL_START}write /tmp/example.txt
This is example content
that spans multiple lines
in the file.
{TOOL_END}

### Patch
Update file content by replacing text:
{TOOL_START}patch [filepath]
{PATCH_DELIMITER_BEFORE}
[text before change]
{PATCH_DELIMITER_AFTER}
[text after change]
{PATCH_DELIMITER_END}
{TOOL_END}

Example:
{TOOL_START}patch /tmp/example.txt
{PATCH_DELIMITER_BEFORE}
old text to replace
{PATCH_DELIMITER_AFTER}
new replacement text
{PATCH_DELIMITER_END}
{TOOL_END}

### Done
Signal task completion with optional summary:
{TOOL_START}done
[summary on multiple lines]
{TOOL_END}

Use this tool when a task is complete to provide a final summary and end the conversation.

Example:
{TOOL_START}done
Task completed. Created new file and configured settings.
All requested changes have been implemented successfully.
{TOOL_END}

## Important Notes
- For tools with complex content (write, patch, done), always place the content on new lines
- When a tool is used, the result will be shown after the tool invocation, not replacing it
"#;