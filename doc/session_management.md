# Session Management in AutoSWE

AutoSWE now provides robust session management capabilities, allowing you to save your conversations and resume them later. This is particularly useful for:

- Continuing work on long-running projects
- Preserving important context between different work sessions
- Maintaining multiple separate conversations for different projects

## Session Features

Each saved session includes:

- Complete conversation history
- System prompt configuration
- Model settings
- Timestamps and metadata
- Message counts and token usage stats

## Commands

| Command | Description |
|---------|-------------|
| `/save [name]` | Save the current session with an optional descriptive name |
| `/load name_or_id` | Load a session by its name or ID |
| `/sessions` | List all available saved sessions |
| `/resume` | Resume the most recently active session |

## Command-Line Options

```
--resume    Automatically resume the last active session on startup
```

## How Sessions Are Stored

Sessions are stored as JSON files in the `.autoswe_sessions` directory. Each session has a unique ID based on its creation timestamp. The most recent session ID is stored in the `.autoswe_last_session` file.

## Examples

### Saving a session

```
> /save my_project
Session saved successfully with ID: session_1645278945
You can load it later with: /load session_1645278945
```

### Listing available sessions

```
> /sessions

Available sessions:
ID                   NAME                           DATE            MESSAGES  
---------------------------------------------------------------------------
session_1645278945   my_project                     2022-02-19 15:30   42     
session_1645198532   bug_fixes                      2022-02-18 12:22   18     
session_1645101234   feature_design                 2022-02-17 09:20   64     
```

### Loading a session

You can load a session by its ID:
```
> /load session_1645278945
Session 'session_1645278945' loaded successfully
```

Or more conveniently, by its name:
```
> /load my_project
Session 'my_project' loaded successfully
```

### Resuming the last session

```
> /resume
Last session resumed successfully
```

## Automatic Resumption

You can automatically resume your last session by using the `--resume` command-line flag:

```
AutoSWE --resume
```

This will start AutoSWE and immediately load your most recent session, allowing you to continue exactly where you left off.

## Best Practices

1. **Save Frequently**: Save important sessions with descriptive names
2. **Use Tags**: Group related sessions with similar names
3. **Clean Up**: Delete old or unneeded sessions to save disk space
4. **Auto-Resume**: Use `--resume` for long-running projects

## Technical Details

Sessions are serialized to JSON format and include:
- Full conversation history
- System prompt configuration
- Model settings
- Metadata (timestamps, message counts, etc.)

When a session is loaded, any cached state is reset to ensure consistency.