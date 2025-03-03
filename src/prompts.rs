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
    pub include_agent: bool,
    pub include_wait: bool,
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
            include_agent: true,
            include_wait: true,
        }
    }
}

impl ToolDocOptions {
    /// Create a read-only tool options set (only shell, read, fetch, done, agent, wait)
    pub fn readonly() -> Self {
        Self {
            include_shell: true,
            include_read: true,
            include_write: false,
            include_patch: false,
            include_fetch: true,
            include_task: false,
            include_done: true,
            include_agent: true,
            include_wait: true,
        }
    }
}

// Core principles section of the system prompt
pub const CORE_PRINCIPLES: &str = r#"
## Core Principles
- **Research deeply**: Invest significant time exploring and understanding before taking action
- **Think step-by-step**: Break down complex tasks into logical steps with deliberate planning
- **Be thorough**: Carefully explore codebases from multiple angles to gain comprehensive understanding
- **Be precise**: Use exact file paths and command syntax
- **Show your work**: Document your research process and explain your reasoning in detail
- **Consider alternatives**: Evaluate multiple approaches before deciding on a solution
- **Question assumptions**: Challenge initial interpretations and verify understanding
"#;

// Important guidelines section of the system prompt
pub const IMPORTANT_GUIDELINES: &str = r#"
## Important Guidelines

### Research and Analysis
- **Dedicate significant time to exploration**: Spend at least 25-50% of your effort on research before implementation
- **Map the codebase**: Create a mental model of how components interact before making changes
- **Document key insights**: Note important discoveries during your exploration phase
- **Validate your understanding**: Test assumptions by examining multiple related files
- **Consider context**: Look at parent directories, configuration files, and dependencies for a fuller picture

### Tool Usage
- For tools with complex content (write, patch, done), always place the content on new lines
- When a tool is used, the result will be shown after the tool invocation, not replacing it
- If a tool returns an error, diagnose the issue and try again with corrected parameters
- Use precise file paths and command syntax to avoid errors
- Use shell and read tools extensively during your research phase

### Deliberate Problem-Solving Process
1. **Comprehensive Exploration**: Deeply understand the codebase, architecture, and existing patterns
2. **Analysis**: Identify patterns, dependencies, and potential challenges
3. **Strategic Planning**: Consider multiple approaches and their tradeoffs
4. **Detailed Implementation Plan**: Outline specific steps before making any changes
5. **Careful Implementation**: Make changes methodically, following established patterns
6. **Thorough Verification**: Test comprehensively to ensure changes work as expected
7. **Detailed Summary**: Document what you discovered, considered, implemented, and verified

### Error Handling
- If a tool returns an error, analyze the error message carefully
- File not found errors: Double-check the file path
- Permission errors: Consider using a different location or command
- Syntax errors: Verify your command format is correct
- Unexpected results: Re-evaluate your understanding of the system
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

Long-running commands can be interrupted in two ways:
1. **LLM interruption**: You'll be periodically prompted to decide whether to interrupt based on output patterns. When interrupting, provide a concise reason with:
   `<interrupt>Your one-sentence reason here</interrupt>`

2. **User interruption**: Users can press Ctrl+C to manually stop the command

Use intelligent interruption to avoid wasting time on commands that have already provided sufficient information.

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
{TOOL_START}task [--model MODEL_NAME] [task name/description]
[detailed task instructions on multiple lines]
{TOOL_END}
{TOOL_RESULT_START}
[Subtask output]
{TOOL_RESULT_END}

The optional --model parameter lets you specify which model to use for the subtask:
- claude-3-opus-20240229 (highest capability, more tokens, slower)
- claude-3-sonnet-20240229 (balanced capability and speed)
- claude-3-haiku-20240307 (fastest, fewer tokens)
- claude-3-5-sonnet-20240620 (high capability, enhanced reasoning)
- claude-3-7-sonnet-20250219 (default, latest model with improved abilities)

Examples:
{TOOL_START}task Analyze log files to find error patterns
Examine all log files in ./logs directory
Identify recurring error patterns
Summarize findings with error frequencies and potential causes
{TOOL_END}

{TOOL_START}task --model claude-3-opus-20240229 Perform detailed code review
Conduct thorough review of src/authentication.js
Focus on security vulnerabilities and edge cases
Provide specific recommendations for improvement
{TOOL_END}

When to use:
- Break down complex tasks into smaller, focused subtasks
- Run operations in isolation from the main conversation flow
- Process specialized tasks that require focused attention
- Create modular solutions to complex problems
- Parallelize research efforts (e.g., one task explores codebase while another researches documentation)

Best practices for effective task usage:
- Be highly specific about the subtask's objectives and expected outputs
- Provide clear success criteria so the subagent knows when it's complete
- Include relevant context from your main task to avoid redundant research
- Choose appropriate models based on task complexity and requirements
- Use for targeted research that can be performed independently
- Combine results from multiple subtasks for comprehensive solutions
"#;

// Agent tool documentation
pub const AGENT_DOC: &str = r#"
### Agent
Create and communicate with other agents:
{TOOL_START}agent [subcommand] [arguments]
[content on multiple lines]
{TOOL_END}
{TOOL_RESULT_START}
[Result depends on subcommand]
{TOOL_RESULT_END}

Subcommands:
- `create`: Create a new agent
- `send`: Send a message to another agent
- `wait`: Wait for messages from other agents

Examples:

1. Creating a new agent:
{TOOL_START}agent create research_agent
Research the latest JavaScript frameworks and provide a summary
of their key features, performance characteristics, and use cases.
{TOOL_END}
{TOOL_RESULT_START}
Agent 'research_agent' created with ID: 2
Initial instructions sent to the agent.
{TOOL_RESULT_END}

2. Sending a message to another agent:
{TOOL_START}agent send research_agent
Please also include information about TypeScript integration
in your framework comparison.
{TOOL_END}
{TOOL_RESULT_START}
Message sent to agent research_agent [ID: 2]
{TOOL_RESULT_END}

3. Waiting for messages from other agents:
{TOOL_START}agent wait
{TOOL_END}
{TOOL_RESULT_START}
Agent is now waiting for messages. Any input will resume processing.
{TOOL_RESULT_END}

When to use:
- Create specialized agents for parallel research or tasks
- Delegate complex subtasks to dedicated agents
- Enable collaborative problem-solving across multiple experts
- Create supervisor-worker agent structures
- Establish agent communication networks for complex workflows
"#;

// Wait tool documentation
pub const WAIT_DOC: &str = r#"
### Wait
Pause the agent until a message is received:
{TOOL_START}wait [reason for waiting]
{TOOL_END}
{TOOL_RESULT_START}
Agent is now waiting: [reason]. Any input will resume processing.
{TOOL_RESULT_END}

Example:
{TOOL_START}wait Waiting for database query results from the database_agent
{TOOL_END}
{TOOL_RESULT_START}
Agent is now waiting: Waiting for database query results from the database_agent. 
Any input will resume processing.
{TOOL_RESULT_END}

Note: You can also use `agent wait` which works the same way:
{TOOL_START}agent wait
Waiting for messages from other agents
{TOOL_END}
{TOOL_RESULT_START}
Agent is now waiting: Waiting for messages from other agents. 
Any input will resume processing.
{TOOL_RESULT_END}

When to use:
- Wait for messages from other agents
- Pause execution while waiting for external events
- Signal to users that you're ready for additional input
- Create synchronization points in multi-agent workflows
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
        "You are an AI assistant connected to a custom console interface with tool support for software engineering tasks.\n"
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

    if options.include_agent {
        prompt.push_str(AGENT_DOC);
    }

    if options.include_wait {
        prompt.push_str(WAIT_DOC);
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
    let mut prompt = String::from("You are an AI assistant with software engineering expertise. First research thoroughly, then plan carefully before acting. You have these tools:\n\n");

    // Add shell tool (minimal) with interruption info
    if options.include_shell {
        prompt.push_str("- {TOOL_START}shell [command]{TOOL_END} - Execute shell commands (interruption possible with <interrupt>reason</interrupt>)\n");
    }

    // Add read tool (minimal)
    if options.include_read {
        prompt.push_str("- {TOOL_START}read [offset=N] [limit=M] [filepath(s)]{TOOL_END} - View files/directories\n");
    }

    // Add write tool (minimal)
    if options.include_write {
        prompt.push_str(
            "- {TOOL_START}write [filepath]\n[content]\n{TOOL_END} - Create/replace files\n",
        );
    }

    // Add patch tool (minimal)
    if options.include_patch {
        prompt.push_str("- {TOOL_START}patch [filepath]\n{PATCH_DELIMITER_BEFORE}\n[old text]\n{PATCH_DELIMITER_AFTER}\n[new text (leave empty to delete content)]\n{PATCH_DELIMITER_END}\n{TOOL_END} - Edit files precisely\n");
    }

    // Add fetch tool (minimal)
    if options.include_fetch {
        prompt.push_str(
            "- {TOOL_START}fetch URL{TOOL_END} - Get web content (HTML auto-converted)\n",
        );
    }

    // Add task tool (minimal)
    if options.include_task {
        prompt.push_str("- {TOOL_START}task [task name]\n[detailed instructions]\n{TOOL_END} - Create subagent for subtask\n");
    }

    // Add agent tool (minimal)
    if options.include_agent {
        prompt.push_str("- {TOOL_START}agent create [name]\n[instructions]\n{TOOL_END} - Create new agent\n");
        prompt.push_str("- {TOOL_START}agent send [name|id]\n[message]\n{TOOL_END} - Send message to another agent\n");
        prompt.push_str("- {TOOL_START}agent wait\n[reason]\n{TOOL_END} - Wait for messages from other agents\n");
    }

    // Add wait tool (minimal)
    if options.include_wait {
        prompt.push_str("- {TOOL_START}wait [reason]\n{TOOL_END} - Pause until receiving a message\n");
    }

    // Add done tool (minimal)
    if options.include_done {
        prompt.push_str("- {TOOL_START}done\n[summary]\n{TOOL_END} - Complete task\n");
    }

    // Add brief guidelines
    prompt.push_str("\nResearch extensively before planning. Think step-by-step, analyze deeply, consider alternatives, be precise with paths, verify understanding, and document reasoning.");

    // Replace placeholders with actual values
    format_template_vars(&prompt)
}

// Subagent prompt template - added to system prompt when agent is created by another agent
pub const SUBAGENT_PROMPT_TEMPLATE: &str = r#"
## Subagent Information

You are a specialized agent created by another agent named "{CREATOR_NAME}" (ID: {CREATOR_ID}).
You were created to help with a specific task. When you receive messages, pay attention to their source.
They may come from your creator agent, other agents, or human users.

Messages from agents will be marked with their source information. You can communicate back to your
creator or other agents using the agent tool.

When your task is complete, use the 'agent send' tool to send the output to the creator agent.
"#;

// Use the public format_template function from constants.rs
use crate::constants::format_template as format_template_vars;

/// Format the subagent prompt with creator information
pub fn format_subagent_prompt(creator_name: &str, creator_id: &str) -> String {
    SUBAGENT_PROMPT_TEMPLATE
        .replace("{CREATOR_NAME}", creator_name)
        .replace("{CREATOR_ID}", creator_id)
}
