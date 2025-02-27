# AutoSWE - Claude API Console Interface

A simple command-line interface for interacting with Claude AI via Anthropic's API.

## Features

- Interactive console-based conversation with Claude AI
- Single query mode for scripting and command-line use
- Maintains conversation history during interactive sessions
- Support for system prompts
- Ability to switch between different Claude models
- Simple command system for interactive control
- .env file support for API keys and configuration

## Requirements

- Rust (latest stable version recommended)
- Anthropic API key

## Installation

1. Clone this repository:
   ```
   git clone <repository-url>
   cd autoswe
   ```

2. Build the project:
   ```
   cargo build --release
   ```

3. Set your Anthropic API key using one of these methods:
   
   - Create a `.env` file in the project root based on the `.env.example` template:
     ```
     ANTHROPIC_API_KEY=your_actual_api_key
     ```
   
   - Or set as an environment variable:
     ```
     export ANTHROPIC_API_KEY=your_actual_api_key
     ```

## Usage

### Interactive Mode

Run the program without arguments to start an interactive session:
```
cargo run --release
```

### Non-Interactive Mode

Provide a query as a command-line argument to get a single response:
```
cargo run --release -- "What is the capital of France?"
```

You can also specify a model and system prompt:
```
cargo run --release -- --model claude-3-haiku-20240307 --system "You are a helpful assistant." "What is the capital of France?"
```

### Command-Line Options

```
Usage: AutoSWE [OPTIONS] [QUERY]

If QUERY is provided, runs in non-interactive mode and outputs only the response.
If QUERY is not provided, starts an interactive console session.

Options:
  --model MODEL_NAME     Specify the Claude model to use
                         (default: claude-3-opus-20240229)
  --system PROMPT        Provide a system prompt for Claude
  --help                 Display this help message

Environment Variables:
  ANTHROPIC_API_KEY      Your Anthropic API key (required)

Example:
  AutoSWE --model claude-3-haiku-20240307 "What is the capital of France?"
```

### Interactive Commands

In interactive mode, the following commands are available:

- `/help` - Displays available commands
- `/clear` - Clears conversation history
- `/system TEXT` - Sets a system prompt
- `/model NAME` - Changes the Claude model version (e.g., claude-3-opus-20240229)
- `/exit` - Exits the program

### Example Models

- `claude-3-opus-20240229` - Most capable model (default)
- `claude-3-sonnet-20240229` - Balanced model
- `claude-3-haiku-20240307` - Fastest model

## Environment File Support

The application supports loading configuration from `.env` files using the dotenvy crate. It will check for `.env` files in the following locations:

1. The project root directory
2. `./env/.env`
3. `../.env`
4. `~/.env`

Example `.env` file content:
```
ANTHROPIC_API_KEY=your_api_key_here
```

## How It Works

This application uses Anthropic's Messages API to send and receive messages to Claude. In interactive mode, it maintains a conversation history in memory, allowing for multi-turn conversations. The system prompt provides high-level instructions to Claude that persist across the conversation.

In non-interactive mode, it sends a single message and returns the response without any formatting, making it suitable for use in scripts and command-line pipes.

## License

MIT