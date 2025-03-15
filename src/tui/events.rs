//! Event handling for the Terminal UI

use crate::agent::AgentMessage;
use crate::tui::{commands, state::TuiState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::time::{Duration, Instant};

/// Handle key events
pub async fn handle_key_event(
    state: &mut TuiState,
    key: KeyEvent,
) -> anyhow::Result<()> {
    match key.code {
        // Multi-level interrupt with Ctrl+C
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            handle_ctrl_c_interrupt(state).await?;
        }

        // Submit on Enter or insert newline with Shift+Enter
        KeyCode::Enter => {
            // If temporary output is visible, dismiss it, reset state, and return
            if state.temp_output.visible {
                state.temp_output.hide();
                // Clear input and hide suggestions when dismissing output
                state.input.clear();
                state.cursor_position = 0;
                state.command_mode = false;
                state.command_suggestions.hide();
                return Ok(());
            }

            // If Shift is held, insert a newline instead of submitting
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                state.input.insert(state.cursor_position, '\n');
                state.cursor_position += 1;
                return Ok(());
            }

            let input = std::mem::take(&mut state.input);
            state.cursor_position = 0;

            // Reset history navigation state
            state.history_index = -1;
            state.current_input = None;

            if !input.is_empty() {
                // Add to command history (if not a duplicate of the last command)
                if state.command_history.last() != Some(&input) {
                    // Add to history, limiting size to 100 entries
                    state.command_history.push(input.clone());
                    if state.command_history.len() > 100 {
                        state.command_history.remove(0);
                    }
                }

                if input.starts_with('/') {
                    // Process slash commands through our dedicated handler
                    commands::process_command(state, &input).await?;

                    // Clear the input after submitting
                    state.input.clear();
                    state.cursor_position = 0;
                    state.command_mode = false;
                } else if input.starts_with('#') {
                    // For pound commands for agent switching, keep the special handling
                    commands::handle_pound_command(state, &input).await?;

                    // Clear the input after submitting
                    state.input.clear();
                    state.cursor_position = 0;
                    state.pound_command_mode = false;
                } else {
                    // Don't add user input to buffer here, agent will handle it
                    // No need to prefix with chevron as the agent will format it properly

                    // Send to selected agent
                    crate::agent::send_message(
                        state.selected_agent_id,
                        AgentMessage::UserInput(input),
                    )?;
                }
            }
        }

        // Backspace
        KeyCode::Backspace => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            if state.cursor_position > 0 {
                state.input.remove(state.cursor_position - 1);
                state.cursor_position -= 1;
                state.update_command_mode();

                // Handle special case: check if we're still in command mode
                if state.command_mode {
                    state
                        .command_suggestions
                        .update_suggestions(&state.input);
                }
            }
        }

        // Delete
        KeyCode::Delete => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            if state.cursor_position < state.input.len() {
                state.input.remove(state.cursor_position);
                state.update_command_mode();

                // Update command suggestions if still in command mode
                if state.command_mode {
                    state
                        .command_suggestions
                        .update_suggestions(&state.input);
                }
            }
        }

        // Left arrow (with modifiers for macOS conventions)
        KeyCode::Left => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            // Command + Left: Move to beginning of line (macOS convention)
            if key.modifiers.contains(KeyModifiers::META) {
                state.cursor_position = 0;
            }
            // Option/Alt + Left: Move one word left (macOS convention)
            else if key.modifiers.contains(KeyModifiers::ALT) {
                // Find previous word boundary
                if state.cursor_position > 0 {
                    // First skip any spaces directly to the left
                    let mut pos = state.cursor_position;
                    let chars: Vec<char> = state.input.chars().collect();

                    // Skip spaces backward
                    while pos > 0 && chars[pos - 1].is_whitespace() {
                        pos -= 1;
                    }

                    // Then skip non-spaces backward (the word)
                    while pos > 0 && !chars[pos - 1].is_whitespace() {
                        pos -= 1;
                    }

                    state.cursor_position = pos;
                }
            }
            // Regular left arrow: Move one character left
            else if state.cursor_position > 0 {
                state.cursor_position -= 1;
            }
        }

        // Right arrow (with modifiers for macOS conventions)
        KeyCode::Right => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            // Command + Right: Move to end of line (macOS convention)
            if key.modifiers.contains(KeyModifiers::META) {
                state.cursor_position = state.input.len();
            }
            // Option/Alt + Right: Move one word right (macOS convention)
            else if key.modifiers.contains(KeyModifiers::ALT) {
                // Find next word boundary
                if state.cursor_position < state.input.len() {
                    let mut pos = state.cursor_position;
                    let chars: Vec<char> = state.input.chars().collect();

                    // Skip non-spaces forward (current word)
                    while pos < chars.len() && !chars[pos].is_whitespace() {
                        pos += 1;
                    }

                    // Then skip spaces forward
                    while pos < chars.len() && chars[pos].is_whitespace() {
                        pos += 1;
                    }

                    state.cursor_position = pos;
                }
            }
            // Regular right arrow: Move one character right
            else if state.cursor_position < state.input.len() {
                state.cursor_position += 1;
            }
        }

        // Home key handling
        KeyCode::Home => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+Home: Scroll to top/oldest messages (offset = 0)
                state.scroll_offset = 0;
            } else if !state.temp_output.visible {
                // Regular Home: Move cursor to start of input
                // Only if temporary output is not visible
                state.cursor_position = 0;
            }
        }

        // End key handling
        KeyCode::End => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                // Shift+End: Scroll to bottom/newest messages (offset = max)
                state.scroll_to_bottom();
            } else if !state.temp_output.visible {
                // Regular End: Move cursor to end of input
                // Only if temporary output is not visible
                state.cursor_position = state.input.len();
            }
        }

        // Regular character input
        KeyCode::Char(c) => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            // Handle Option+Right (commonly produces 'f' character in macOS terminal - "forward")
            if c == 'f' && key.modifiers.contains(KeyModifiers::ALT) {
                // Move one word right
                if state.cursor_position < state.input.len() {
                    let mut pos = state.cursor_position;
                    let chars: Vec<char> = state.input.chars().collect();

                    // Skip non-spaces forward (current word)
                    while pos < chars.len() && !chars[pos].is_whitespace() {
                        pos += 1;
                    }

                    // Then skip spaces forward
                    while pos < chars.len() && chars[pos].is_whitespace() {
                        pos += 1;
                    }

                    state.cursor_position = pos;
                }
                return Ok(());
            }

            // Handle Option+Left (commonly produces 'b' character in macOS terminal - "backward")
            if c == 'b' && key.modifiers.contains(KeyModifiers::ALT) {
                // Move one word left
                if state.cursor_position > 0 {
                    // First skip any spaces directly to the left
                    let mut pos = state.cursor_position;
                    let chars: Vec<char> = state.input.chars().collect();

                    // Skip spaces backward
                    while pos > 0 && chars[pos - 1].is_whitespace() {
                        pos -= 1;
                    }

                    // Then skip non-spaces backward (the word)
                    while pos > 0 && !chars[pos - 1].is_whitespace() {
                        pos -= 1;
                    }

                    state.cursor_position = pos;
                }
                return Ok(());
            }

            state.input.insert(state.cursor_position, c);
            state.cursor_position += 1;
            state.update_command_mode();

            // Special handling when starting a command - show suggestions immediately
            if state.input == "/" {
                state.command_suggestions.show("/");
            }
        }

        // Tab key for command completion
        KeyCode::Tab => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            // Only handle Tab in command mode with visible suggestions
            if state.command_mode && state.command_suggestions.visible {
                // Get the currently selected command
                if let Some(selected) = state.command_suggestions.selected_command() {
                    // Replace current input with the selected command
                    state.input = selected.name.clone();
                    state.cursor_position = state.input.len();

                    // If there's only one suggestion, add a space for parameters
                    if state.command_suggestions.filtered_commands.len() == 1 {
                        state.input.push(' ');
                        state.cursor_position += 1;
                        // Hide suggestions after completion
                        state.command_suggestions.hide();
                    } else {
                        // More than one suggestion, cycle to next
                        state.command_suggestions.next();
                    }
                }
            }
        }

        // Escape either dismisses temp output, hides suggestions, or clears the input
        KeyCode::Esc => {
            if state.temp_output.visible {
                state.temp_output.hide();
            } else if state.command_suggestions.visible {
                state.command_suggestions.hide();
            } else {
                // Clear input and reset history navigation
                state.input.clear();
                state.cursor_position = 0;
                state.command_mode = false;
                state.history_index = -1;
                state.current_input = None;
            }
        }

        // PageUp/PageDown for scrolling
        KeyCode::PageUp => {
            // Scroll up (showing older messages)
            let scroll_amount = state.visible_height / 2;
            state.scroll(-(scroll_amount as isize));
        }

        KeyCode::PageDown => {
            // Scroll down (showing newer messages)
            let scroll_amount = state.visible_height / 2;
            state.scroll(scroll_amount as isize);
        }

        // Up arrow handling - navigate suggestions, history, or scroll
        KeyCode::Up => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            // If command suggestions are visible, navigate up through them
            if state.command_mode
                && state.command_suggestions.visible
                && !state.command_suggestions.filtered_commands.is_empty()
            {
                // Navigate to previous suggestion (looping to bottom if at top)
                let current = state.command_suggestions.selected_index;
                let count = state.command_suggestions.filtered_commands.len();

                // Calculate previous index with wrap-around
                let prev = if current == 0 { count - 1 } else { current - 1 };
                state.command_suggestions.selected_index = prev;

                // Automatically update input with the currently selected suggestion
                if let Some(selected) = state.command_suggestions.selected_command() {
                    state.input = selected.name.clone();
                    state.cursor_position = state.input.len();
                }
            }
            // Handle as scroll with shift modifier
            else if key.modifiers.contains(KeyModifiers::SHIFT) {
                state.scroll(-1);
            }
            // If in normal input mode, navigate command history
            else if !state.command_history.is_empty() {
                // Save current input when starting history navigation
                if state.history_index == -1 {
                    state.current_input = Some(state.input.clone());
                }

                // Go backward in history if not at beginning
                if state.history_index < (state.command_history.len() as isize - 1) {
                    state.history_index += 1;
                    let history_entry =
                        &state.command_history[state.command_history.len()
                            - 1
                            - state.history_index as usize];
                    state.input = history_entry.clone();
                    state.cursor_position = state.input.len();
                }
            }
        }

        // Down arrow handling - navigate suggestions, history, or scroll
        KeyCode::Down => {
            // Ignore if temporary output is visible
            if state.temp_output.visible {
                return Ok(());
            }

            // If command suggestions are visible, navigate down through them
            if state.command_mode
                && state.command_suggestions.visible
                && !state.command_suggestions.filtered_commands.is_empty()
            {
                // Navigate to next suggestion (looping to top if at bottom)
                state.command_suggestions.next();

                // Automatically update input with the currently selected suggestion
                if let Some(selected) = state.command_suggestions.selected_command() {
                    state.input = selected.name.clone();
                    state.cursor_position = state.input.len();
                }
            }
            // Handle as scroll with shift modifier
            else if key.modifiers.contains(KeyModifiers::SHIFT) {
                state.scroll(1);
            }
            // If currently navigating history, go forward
            else if state.history_index > -1 {
                state.history_index -= 1;

                // If reached beyond the most recent history item, restore the original input
                if state.history_index == -1 {
                    if let Some(original_input) = state.current_input.take() {
                        state.input = original_input;
                    } else {
                        state.input.clear();
                    }
                } else {
                    // Otherwise show the history entry
                    let history_entry =
                        &state.command_history[state.command_history.len()
                            - 1
                            - state.history_index as usize];
                    state.input = history_entry.clone();
                }
                state.cursor_position = state.input.len();
            }
        }

        // Ignore other keys
        _ => {}
    }

    Ok(())
}

/// Handle mouse events
pub async fn handle_mouse_event(
    state: &mut TuiState,
    mouse: MouseEvent,
) -> anyhow::Result<()> {
    // Simple mouse wheel scrolling implementation
    match mouse.kind {
        MouseEventKind::ScrollDown => {
            // Scroll down (increase offset to show newer/more recent messages)
            state.scroll(3);
        }
        MouseEventKind::ScrollUp => {
            // Scroll up (decrease offset to show older messages)
            state.scroll(-3);
        }
        _ => {}
    }

    Ok(())
}

/// Handle Ctrl+C interrupt with multi-level behavior
async fn handle_ctrl_c_interrupt(
    state: &mut TuiState,
) -> anyhow::Result<()> {
    // Define the double-press window (3 seconds)
    const DOUBLE_PRESS_WINDOW: Duration = Duration::from_secs(3);

    // Get current time
    let now = Instant::now();

    // Check if this is a double-press (second Ctrl+C within window)
    if let Some(last_time) = state.last_interrupt_time {
        // Only count as double-press if previous Ctrl+C wasn't for interrupting a process
        if !state.last_interrupt_was_process
            && now.duration_since(last_time) < DOUBLE_PRESS_WINDOW
        {
            // This is a double-press, exit the application
            let popup_title = "Exiting Application".to_string();
            let popup_content = "Received second Ctrl+C. Exiting application...".to_string();
            commands::show_command_result(state, popup_title, popup_content);

            state.should_quit = true;
            return Ok(());
        }
    }

    // Get current agent state
    let agent_state = crate::agent::get_agent_state(state.selected_agent_id).ok();

    let popup_title = "Interrupt".to_string();
    let popup_content;

    match agent_state {
        // If running a shell command (interruptible tool) or if agent is actively processing
        Some(crate::agent::AgentState::RunningTool { .. }) | Some(crate::agent::AgentState::Processing) => {
            // Interrupt the agent
            crate::agent::interrupt_agent_with_reason(
                state.selected_agent_id,
                "User pressed Ctrl+C".to_string(),
            )?;

            // Mark that we used Ctrl+C to interrupt a process
            // This prevents it from counting towards the double-press exit timer
            state.last_interrupt_time = Some(now);
            state.last_interrupt_was_process = true;

            // Don't show popup when interrupting an agent
            // Just silently interrupt the process
        }

        // If agent is waiting for input (idle or done), start the double-press timer
        _ => {
            popup_content =
                "Press Ctrl+C again within 3 seconds to exit application.".to_string();

            // Start the double-press timer for exiting the application
            state.last_interrupt_time = Some(now);
            state.last_interrupt_was_process = false;

            // Only show popup when we're counting toward application exit
            commands::show_command_result(state, popup_title, popup_content);
        }
    }

    Ok(())
}