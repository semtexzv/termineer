# AutoSWE - Multi-LLM Console Interface

A powerful command-line interface for interacting with various AI models including Claude and Google Gemini.

## Features

- Interactive console-based conversation with multiple AI models
- Support for Anthropic Claude, Google Gemini, and OpenRouter models
- Single query mode for scripting and command-line use
- Maintains conversation history during interactive sessions
- Support for system prompts
- Ability to switch between different models
- Advanced tool capabilities including web content summarization
- Simple command system for interactive control
- .env file support for API keys and configuration

## Requirements

- Rust (latest stable version recommended)
- Anthropic API key (for Claude models)
- Google API key (for Gemini models)
- OpenRouter API key (for access to various models through OpenRouter)

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

3. Set up your API keys using one of these methods:
   
   - Create a `.env` file in the project root based on the `.env.example` template:
     ```
     # For Claude models
     ANTHROPIC_API_KEY=your_anthropic_api_key
     
     # For Google Gemini models
     GOOGLE_API_KEY=your_google_api_key
     
     # For OpenRouter models
     OPENROUTER_API_KEY=your_openrouter_api_key
     
     # Optional OpenRouter site information
     OPENROUTER_SITE_URL=https://your-site-url.com
     OPENROUTER_SITE_NAME=Your Site Name
     ```
   
   - Or set as environment variables:
     ```
     # For Claude models
     export ANTHROPIC_API_KEY=your_anthropic_api_key
     
     # For Google Gemini models
     export GOOGLE_API_KEY=your_google_api_key
     
     # For OpenRouter models
     export OPENROUTER_API_KEY=your_openrouter_api_key
     export OPENROUTER_SITE_URL=https://your-site-url.com
     export OPENROUTER_SITE_NAME="Your Site Name"
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
  --model MODEL_NAME     Specify the model to use
                         (default: claude-3-opus-20240229)
  --system PROMPT        Provide a system prompt
  --help                 Display this help message

Environment Variables:
  ANTHROPIC_API_KEY      Your Anthropic API key (required for Claude models)
  GOOGLE_API_KEY         Your Google API key (required for Gemini models)

Example:
  AutoSWE --model claude-3-haiku-20240307 "What is the capital of France?"
  AutoSWE --model google/gemini-1.5-flash "Explain quantum computing."
```

### Interactive Commands

In interactive mode, the following commands are available:

- `/help` - Displays available commands
- `/clear` - Clears conversation history
- `/system TEXT` - Sets a system prompt
- `/model NAME` - Changes the model (e.g., claude-3-opus-20240229 or gemini-1.5-flash)
- `/exit` - Exits the program

### Example Models

#### Anthropic Claude Models
- `claude-3-opus-20240229` - Most capable Claude model (default)
- `claude-3-sonnet-20240229` - Balanced Claude model
- `claude-3-haiku-20240307` - Fastest Claude model
- `claude-3-7-sonnet-20250219` - Latest Claude 3.7 model

#### Google Gemini Models
- `gemini-1.5-flash` - Fast Gemini model
- `gemini-1.5-pro` - Capable Gemini model
- `gemini-pro` - Previous generation Gemini model

#### OpenRouter Models
- `openrouter/gpt-4o` - OpenAI's GPT-4o model via OpenRouter
- `openrouter/anthropic/claude-3-opus` - Claude Opus via OpenRouter (note the provider prefix)
- `openrouter/anthropic/claude-3-haiku` - Claude Haiku via OpenRouter
- `openrouter/anthropic/claude-3-sonnet` - Claude Sonnet via OpenRouter

You can also use explicit provider prefixes:
- `anthropic/claude-3-opus-20240229`
- `google/gemini-1.5-flash`
- `openrouter/gpt-4-turbo`

### Using OpenRouter Models

When using OpenRouter, you may need to use specific model ID formats that match OpenRouter's API. For the most accurate and up-to-date model IDs, check:

```
https://openrouter.ai/docs#models
```

Some models require nested provider paths like `openrouter/anthropic/claude-3-opus` while others use direct naming like `openrouter/gpt-4o`.

#### Implementation Details

The OpenRouter integration provides these features:

1. **Unified Access**: Connect to hundreds of AI models through a single API
2. **Smart Error Handling**: Automatic retry with exponential backoff for rate limits
3. **Model Information Caching**: Optimized performance through caching model details
4. **Direct Header Support**: Set `HTTP-Referer` and `X-Title` headers for OpenRouter rankings

Models are accessed with the `openrouter/` prefix followed by the model ID as shown in OpenRouter's documentation.

## Advanced Features

### Enhanced Web Content Fetching

The fetch tool now includes content summarization capabilities, reducing token usage and focusing on essential information:

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

Summarization uses the Google Gemini Flash model to efficiently process content, saving tokens in your main conversation.

### Context Caching for Gemini Models

This application supports Gemini's context caching, which can improve performance and reduce token usage for similar requests:

- Automatically caches conversation contexts in memory
- Reduces token usage by reusing previous context
- Applies intelligent cache key generation based on conversation content
- Cache entries automatically expire after a configurable TTL (default: 1 hour)

Cache is transparently managed in the background without requiring any user action.

## Environment File Support

The application supports loading configuration from `.env` files using the dotenvy crate. It will check for `.env` files in the following locations:

1. The project root directory
2. `./env/.env`
3. `../.env`
4. `~/.env`

Example `.env` file content:
```
# For Claude models
ANTHROPIC_API_KEY=your_anthropic_api_key

# For Google Gemini models
GOOGLE_API_KEY=your_google_api_key
```

## How It Works

This application connects to multiple AI providers including Anthropic and Google. In interactive mode, it maintains a conversation history in memory, allowing for multi-turn conversations. The system prompt provides high-level instructions that persist across the conversation.

In non-interactive mode, it sends a single message and returns the response without any formatting, making it suitable for use in scripts and command-line pipes.

## License

MIT