// No direct imports needed from constants as we now use format_template directly

/// Defines which tools should be included in the documentation
pub struct ToolDocOptions {
    pub include_shell: bool,
    pub include_read: bool,
    pub include_write: bool,
    pub include_patch: bool,
    pub include_fetch: bool,
    pub include_task: bool,
    pub include_done: bool,
}

impl Default for ToolDocOptions {
    fn default() -> Self {
        Self {
            include_shell: true,
            include_read: true,
            include_write: true,
            include_patch: true,
            include_fetch: true,
            include_task: true,
            include_done: true,
        }
    }
}

impl ToolDocOptions {
    /// Create a read-only tool options set (only shell, read, fetch, done)
    pub fn readonly() -> Self {
        Self {
            include_shell: true,
            include_read: true,
            include_write: false,
            include_patch: false,
            include_fetch: true,
            include_task: false,
            include_done: true,
        }
    }
}

// Core principles section of the system prompt
pub const CORE_PRINCIPLES: &str = r#"
## Core Principles
- **Think step-by-step**: Break down complex tasks into logical steps
- **Be thorough**: Carefully explore codebases before making changes
- **Be precise**: Use exact file paths and command syntax
- **Show your work**: Explain your reasoning and approach
"#;

// Important guidelines section of the system prompt
pub const IMPORTANT_GUIDELINES: &str = r#"
## Important Guidelines

### Tool Usage
- For tools with complex content (write, patch, done), always place the content on new lines
- When a tool is used, the result will be shown after the tool invocation, not replacing it
- If a tool returns an error, diagnose the issue and try again with corrected parameters
- Use precise file paths and command syntax to avoid errors

### Approach to Problems
1. **Explore**: Understand the codebase structure and existing functionality
2. **Plan**: Outline your approach before making changes
3. **Implement**: Make changes carefully, with appropriate testing
4. **Verify**: Confirm your changes work as expected
5. **Summarize**: Use the done tool to explain what you accomplished

### Error Handling
- If a tool returns an error, analyze the error message carefully
- File not found errors: Double-check the file path
- Permission errors: Consider using a different location or command
- Syntax errors: Verify your command format is correct
"#;

// Shell tool documentation
pub const SHELL_DOC: &str = r#"
### Shell
Execute shell commands on the user's system:
{TOOL_START}shell [command]{TOOL_END}

Example:
{TOOL_START}shell ls -la{TOOL_END}
{TOOL_RESULT_START}
drwxr-xr-x  14 mhornicky  primarygroup    448 Feb 27 22:11 .
drwxr-x---+ 89 mhornicky  primarygroup   2848 Feb 27 21:59 ..
-rw-r--r--@  1 mhornicky  primarygroup   2169 Feb 27 22:09 .autoswe
-rw-r--r--   1 mhornicky  primarygroup    127 Feb 27 13:44 .env
-rw-r--r--@  1 mhornicky  primarygroup    175 Feb 27 10:48 .env.example
{TOOL_RESULT_END}
When to use:
- Explore directories and file structures
- Run build commands, tests, or package managers
- Check environment settings or configurations
"#;

// Read tool documentation
pub const READ_DOC: &str = r#"
### Read
Read the contents of files or list directory contents:
{TOOL_START}read [offset=N] [limit=M] [filepath(s) or directory path]{TOOL_END}
- The `offset` parameter (optional) specifies the starting line number (0-indexed)
- The `limit` parameter (optional) specifies the maximum number of lines to read
- Multiple files can be read at once (space-separated) when not using offset/limit
- For large files, only the first and last few lines will be shown
- When a directory path is provided, lists all files and subdirectories in that directory

Examples:
{TOOL_START}read /etc/hosts{TOOL_END}
{TOOL_RESULT_START}
File:  (all {} lines)\n\n```\n{}\n```
127.0.0.1 localhost
::1 localhost ip6-localhost ip6-loopback
fd00:a516:7c1b:17cd:6d81:2137:bd2a:2c5b ip6-localnet
fc00:db20:35b:7399::5 ip6-mcastprefix
fd00:a516:7c1b:17cd:6d81:2137:bd2a:2c5b ip6-allnodes
fd00:a516:7c1b:17cd:6d81:2137:bd2a:2c5b ip6-allrouters
192.168.1.100 ubuntu-vm
{TOOL_RESULT_END}
{TOOL_START}read offset=0 limit=1 /etc/hosts{TOOL_END}
{TOOL_RESULT_START}
127.0.0.1 localhost
{TOOL_RESULT_END}
{TOOL_START}read file1.txt file2.txt file3.txt{TOOL_END}
{TOOL_START}read /path/to/directory{TOOL_END}

When to use:
- Examine code, configuration files, logs, or documentation
- List directory contents to explore project structure
- Understand existing implementations before making changes
- Compare multiple related files at once
- Verify changes after they've been made
"#;

// Write tool documentation
pub const WRITE_DOC: &str = r#"
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
{TOOL_RESULT_START}
File written successfully.
{TOOL_RESULT_END}

When to use:
- Create new files (scripts, configs, documentation)
- Generate complete replacements for existing files
- Output results or generate reports
"#;

// Patch tool documentation
pub const PATCH_DOC: &str = r#"
### Patch
Update file content by replacing text:
{TOOL_START}patch [filepath]
{PATCH_DELIMITER_BEFORE}
[text before change]
{PATCH_DELIMITER_AFTER}
[text after change]
{PATCH_DELIMITER_END}
{TOOL_END}
{TOOL_RESULT_START}
File patched successfully.
{TOOL_RESULT_END}

Example:
{TOOL_START}patch /tmp/example.txt
{PATCH_DELIMITER_BEFORE}
old text to replace
{PATCH_DELIMITER_AFTER}
new replacement text
{PATCH_DELIMITER_END}
{TOOL_END}
{TOOL_RESULT_START}
File patched successfully.
{TOOL_RESULT_END}

Deletion example:
{TOOL_START}patch /tmp/example.txt
{PATCH_DELIMITER_BEFORE}
old text to replace
{PATCH_DELIMITER_AFTER}
{PATCH_DELIMITER_END}
{TOOL_END}
{TOOL_RESULT_START}
File patched successfully.
{TOOL_RESULT_END}

When to use:
- Make targeted changes to specific sections of code
- Fix bugs or implement new features in existing files
- Update configuration settings without rewriting entire files
"#;

// Fetch tool documentation
pub const FETCH_DOC: &str = r#"
### Fetch
Retrieve content from web URLs:
{TOOL_START}fetch URL{TOOL_END}

Example:
{TOOL_START}fetch https://example.com{TOOL_END}
{TOOL_RESULT_START}
Fetched from https://example.com:

# Example Domain

This domain is for use in illustrative examples in documents.
{TOOL_RESULT_END}

When to use:
- Retrieve documentation from external sources
- Get information from public APIs
- Access web content (HTML is automatically converted to readable text)
- Incorporate external reference material into your work
"#;

// Task tool documentation
pub const TASK_DOC: &str = r#"
### Task
Create a subagent to handle a specific subtask:
{TOOL_START}task [task name/description]
[detailed task instructions on multiple lines]
{TOOL_END}
{TOOL_RESULT_START}
[Subtask output]
{TOOL_RESULT_END}

When to use:
- Break down complex tasks into smaller, focused subtasks
- Run operations in isolation from the main conversation flow
- Process specialized tasks that require focused attention
- Create modular solutions to complex problems
"#;

// Done tool documentation
pub const DONE_DOC: &str = r#"
### Done
Signal task completion with optional summary:
{TOOL_START}done
[summary on multiple lines]
{TOOL_END}
{TOOL_RESULT_START}{TOOL_RESULT_END}

Use this tool when a task is complete to provide a final summary and end the conversation.

Example:
{TOOL_START}done
Task completed. Created new file and configured settings.
All requested changes have been implemented successfully.
{TOOL_END}
{TOOL_RESULT_START}{TOOL_RESULT_END}
"#;

/// Generate a system prompt with appropriate tool documentation
pub fn generate_system_prompt(options: &ToolDocOptions) -> String {
    let mut prompt = String::from(
        "You are Claude, an AI assistant by Anthropic. You are connected to a custom console interface with tool support for software engineering tasks.\n"
    );
    
    // Add core principles
    prompt.push_str(CORE_PRINCIPLES);
    
    // Add tool documentation
    prompt.push_str("\n## Available Tools\n");
    
    if options.include_shell {
        prompt.push_str(SHELL_DOC);
    }
    
    if options.include_read {
        prompt.push_str(READ_DOC);
    }
    
    if options.include_write {
        prompt.push_str(WRITE_DOC);
    }
    
    if options.include_patch {
        prompt.push_str(PATCH_DOC);
    }
    
    if options.include_fetch {
        prompt.push_str(FETCH_DOC);
    }
    
    if options.include_task {
        prompt.push_str(TASK_DOC);
    }
    
    if options.include_done {
        prompt.push_str(DONE_DOC);
    }
    
    // Add important guidelines
    prompt.push_str(IMPORTANT_GUIDELINES);
    
    // Replace placeholders with actual values
    format_template_vars(&prompt)
}

/// Generate a minimal system prompt with appropriate tool documentation
pub fn generate_minimal_system_prompt(options: &ToolDocOptions) -> String {
    let mut prompt = String::from("You are Claude, an AI assistant with software engineering expertise. You have these tools:\n\n");
    
    // Add shell tool (minimal)
    if options.include_shell {
        prompt.push_str("- {TOOL_START}shell [command]{TOOL_END} - Execute shell commands\n");
    }
    
    // Add read tool (minimal)
    if options.include_read {
        prompt.push_str("- {TOOL_START}read [offset=N] [limit=M] [filepath(s)]{TOOL_END} - View files/directories\n");
    }
    
    // Add write tool (minimal)
    if options.include_write {
        prompt.push_str("- {TOOL_START}write [filepath]\n[content]\n{TOOL_END} - Create/replace files\n");
    }
    
    // Add patch tool (minimal)
    if options.include_patch {
        prompt.push_str("- {TOOL_START}patch [filepath]\n{PATCH_DELIMITER_BEFORE}\n[old text]\n{PATCH_DELIMITER_AFTER}\n[new text (leave empty to delete content)]\n{PATCH_DELIMITER_END}\n{TOOL_END} - Edit files precisely\n");
    }
    
    // Add fetch tool (minimal)
    if options.include_fetch {
        prompt.push_str("- {TOOL_START}fetch URL{TOOL_END} - Get web content (HTML auto-converted)\n");
    }
    
    // Add task tool (minimal)
    if options.include_task {
        prompt.push_str("- {TOOL_START}task [task name]\n[detailed instructions]\n{TOOL_END} - Create subagent for subtask\n");
    }
    
    // Add done tool (minimal)
    if options.include_done {
        prompt.push_str("- {TOOL_START}done\n[summary]\n{TOOL_END} - Complete task\n");
    }
    
    // Add brief guidelines
    prompt.push_str("\nThink step-by-step, explore before changes, be precise with paths, verify changes.");
    
    // Replace placeholders with actual values
    format_template_vars(&prompt)
}

// Use the public format_template function from constants.rs
use crate::constants::format_template as format_template_vars;