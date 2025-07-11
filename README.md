# Termineer: Advanced AI Terminal Assistant

Termineer is a powerful command-line interface that brings conversational AI capabilities to your terminal, enhancing productivity for developers, data analysts, and power users.

## Overview

Termineer creates a seamless bridge between the terminal environment and state-of-the-art AI models, allowing power users to leverage natural language for a variety of tasks while remaining in their preferred workflow environment.

### Key Capabilities

- **Interactive AI in Your Terminal**: Have multi-turn conversations with AI assistants directly in your command line
- **Multi-Model Support**: Connect to Anthropic Claude, Google Gemini, and OpenRouter models
- **Powerful Tooling**: Execute commands, manipulate files, fetch web content, and more through natural language
- **Terminal Efficiency**: Maintain your productivity environment without switching contexts
- **Conversational Approach**: Inspired by the emerging practice of expressing intent through conversation rather than explicit coding (sometimes called "vibe coding")
- **Rich Terminal UI**: Sophisticated terminal interface with conversation history and tool output display
- **Subscription Tiers**: Free/Plus/Pro modes with different feature sets and model access
- **Multi-Agent Collaboration**: Create and coordinate multiple agents for complex tasks
- **Task Management**: Create subtasks with specialized agent kinds for focused execution
- **Intelligent Interruption**: LLM-based decision-making for interrupting long-running operations

## Requirements

- Rust (latest stable version recommended)
- Anthropic API key (for Claude models)
- Google API key (for Gemini models)
- OpenRouter API key (optional, for access to various models through OpenRouter)

## Project Structure

The Termineer project is organized into several key components:

- **Core Application**: Main program logic, configuration, and UI implementation
- **Agent System**: Agent implementation with conversation management and LLM interaction
- **Tools Implementation**: Comprehensive tool suite for environment interaction
- **LLM Integration**: Interface for communicating with different AI model providers
- **Model Context Protocol (MCP)**: Support for extending agent capabilities via MCP
- **Authentication**: OAuth-based authentication system for subscription management
- **Server Component**: Backend server for authentication, subscriptions, and API services

## Installation

1. Clone this repository:
   ```
   git clone <repository-url>
   cd termineer
   ```

2. Build the project:
   ```
   cargo build --release
   ```

3. Set up your API keys using one of these methods:
   
   - Create a `.env` file in the project root based on the `.env.example` template:
     ```
     # For Claude models
     ANTHROPIC_API_KEY=your_anthropic_api_key
     ```
   
   - Or set as environment variables:
     ```
     # For Claude models
     export ANTHROPIC_API_KEY=your_anthropic_api_key
     ```

## Usage

### Interactive Mode

Run the program without arguments to start an interactive session:
```
cargo run --release
```

### Single Query Mode

Provide a query as a command-line argument to get a single response:
```
cargo run --release -- "Analyze the memory usage patterns in this log file"
```

You can also specify a model and system prompt:
```
cargo run --release -- --model claude-3-haiku-20240307 --system "You are a helpful assistant." "What is the most efficient algorithm for this problem?"
```

### Command-Line Options

```
Usage: Termineer [OPTIONS] [QUERY]

If QUERY is provided, runs in non-interactive mode and outputs only the response.
If QUERY is not provided, starts an interactive console session.

Options:
  --model MODEL_NAME     Specify the model to use
                         (default: claude-3-opus-20240229)
  --system PROMPT        Provide a system prompt
  --help                 Display this help message

Environment Variables:
  ANTHROPIC_API_KEY      Your Anthropic API key (required for Claude models)
  GOOGLE_API_KEY         Your Google API key (required for Gemini models)

Example:
  Termineer --model claude-3-haiku-20240307 "What is the capital of France?"
  Termineer --model google/gemini-1.5-flash "Explain quantum computing."
```

### Interactive Commands

In interactive mode, the following commands are available:

- `/help` - Displays available commands
- `/clear` - Clears conversation history
- `/system TEXT` - Sets a system prompt
- `/model NAME` - Changes the model (e.g., claude-3-opus-20240229 or gemini-1.5-flash)
- `/exit` - Exits the program

## Available AI Models

### Anthropic Claude Models
- `claude-3-opus-20240229` - Most capable Claude model (default)
- `claude-3-sonnet-20240229` - Balanced Claude model
- `claude-3-haiku-20240307` - Fastest Claude model
- `claude-3-7-sonnet-20250219` - Latest Claude 3.7 model

### Google Gemini Models
- `gemini-1.5-flash` - Fast Gemini model
- `gemini-1.5-pro` - Capable Gemini model
- `gemini-pro` - Previous generation Gemini model

### OpenRouter Models
- `openrouter/gpt-4o` - OpenAI's GPT-4o model via OpenRouter
- `openrouter/anthropic/claude-3-opus` - Claude Opus via OpenRouter (note the provider prefix)
- `openrouter/anthropic/claude-3-haiku` - Claude Haiku via OpenRouter
- `openrouter/anthropic/claude-3-sonnet` - Claude Sonnet via OpenRouter

## Advanced Features

### MCP (Model Context Protocol) Integration

Termineer supports the Model Context Protocol for enhanced tool capabilities. MCP servers are configured using a `.term/config.json` file in your project directory:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": [
        "-y",
        "@modelcontextprotocol/server-filesystem",
        "/Users/username/Desktop",
        "/Users/username/Downloads"
      ]
    }
  }
}
```

This configuration is loaded automatically on startup, and the configured MCP servers are made available to the AI by including them in the system prompt.

For detailed instructions, see:
- [MCP Configuration Guide](./docs/mcp-configuration.md)
- [MCP Template Integration](./docs/mcp-template-integration.md)

### Enhanced Web Content Fetching

The fetch tool includes content summarization capabilities, reducing token usage and focusing on essential information:

```
fetch [--summarize] [--length short|medium|long|<word_count>] https://example.com
```

Options:
- `--summarize` or `-s`: Enable summarization of the webpage content
- `--length` or `-l`: Specify summary length (default is medium)
  - `short`: ~150 words
  - `medium`: ~400 words
  - `long`: ~800 words
  - Custom word count (e.g., `500`): Target specific length

Examples:
```
fetch https://example.com
fetch --summarize https://example.com
fetch --summarize --length long https://example.com
fetch --summarize --length 300 https://example.com
```

## Use Cases

Termineer excels in a variety of scenarios:

### For Developers
- Generate boilerplate code with a single prompt
- Debug complex issues by describing the problem
- Create prototypes rapidly
- Automate repetitive development tasks
- Research APIs and integration patterns

### For Data Analysts
- Generate analysis scripts
- Find patterns in complex datasets
- Create and optimize queries
- Extract insights from raw data

### For System Administrators
- Generate complex shell commands
- Create scripts for system maintenance
- Debug configuration issues
- Document system setups

### For Content Creators
- Research topics efficiently
- Generate outlines and drafts
- Summarize complex content
- Edit and refine writing

## Environment File Support

The application supports loading configuration from `.env` files using the dotenvy crate. It will check for `.env` files in the following locations:

1. The project root directory
2. `./env/.env`
3. `../.env`
4. `~/.env`

## How It Works

Termineer connects to multiple AI providers including Anthropic and Google. In interactive mode, it maintains a conversation history in memory, allowing for multi-turn conversations. The system prompt provides high-level instructions that persist across the conversation.

In non-interactive mode, it sends a single message and returns the response without any formatting, making it suitable for use in scripts and command-line pipes.

## About the Name

"Termineer" combines "terminal" and "engineer" - representing an AI-powered engineering assistant that lives in your terminal, ready to help with your tasks.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Git Hooks (Recommended)

This repository includes a `pre-push` Git hook that automatically runs all tests before you can push your changes. This helps ensure that broken code is not accidentally pushed to the repository.

**To install the hook**, run the following command from the root of the repository:

```bash
ln -s -f ../../scripts/pre-push .git/hooks/pre-push
```

This creates a symbolic link from your local Git hooks directory to the version-controlled script.

### Viewing Release Notes

Since this is a private repository, release notes are available on the [GitHub Releases](https://github.com/semtexzv/termineer/releases) page for authenticated users with access.

### Releasing a New Version

To create a new release, use the `release.sh` script:

```bash
./release.sh
```

This will automatically increment the patch version, update the necessary files, and push a new git tag to trigger the release workflow.

To specify a version, use the `-v` flag:

```bash
./release.sh -v 0.3.0
```

## License

MIT

## Testing The Patch Tool

This section was successfully updated using the Termineer patch tool to demonstrate its functionality! The patch tool allows for precise updates to files by:

1. Specifying the exact text to replace (including whitespace and indentation)
2. Providing the new text to insert instead
3. Applying the changes with proper context validation
4. Ensuring the changes are applied correctly at the intended location

When using the patch tool, remember to include sufficient context around your changes to ensure uniqueness within the file. This helps the tool identify exactly where changes should be applied, particularly in large files with similar sections.

### Patch Tool Best Practices

- Include 3-5 lines of context around your changes
- Match whitespace and indentation exactly
- Test patches on small sections before making large changes
- Verify the file after patching to ensure changes were applied correctly