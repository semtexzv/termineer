//! State management for the Terminal UI

use crate::agent::{AgentId, AgentManager, AgentState};
use crate::output::SharedBuffer;
use crate::tui::popup::{CommandSuggestionsPopup, TemporaryOutput};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Maximum number of lines to keep in the conversation history view
#[allow(dead_code)]
pub const MAX_HISTORY_LINES: usize = 1000;

/// State for the TUI application
pub struct TuiState {
    /// Input being typed by the user
    pub input: String,
    /// Cursor position in the input field
    pub cursor_position: usize,
    /// Currently selected agent ID
    pub selected_agent_id: AgentId,
    /// Buffer for the selected agent's output.
    pub agent_buffer: SharedBuffer,
    /// Whether the application should exit
    pub should_quit: bool,
    /// Command mode indicator (when input starts with '/')
    pub command_mode: bool,
    /// Pound command mode indicator (when input starts with '#')
    pub pound_command_mode: bool,
    /// Last time Ctrl+C was pressed (for double-press exit)
    pub last_interrupt_time: Option<Instant>,
    /// Whether the last Ctrl+C was used to interrupt an agent/shell
    pub last_interrupt_was_process: bool,
    /// Reference to the agent manager
    pub agent_manager: Arc<Mutex<AgentManager>>,
    /// Scroll offset for the conversation view (0 = top of conversation)
    pub scroll_offset: usize,
    /// Maximum scroll offset based on content and view size
    pub max_scroll_offset: usize,
    /// Visible content height in lines
    pub visible_height: usize,
    /// Temporary output window that grows upward from the input area
    pub temp_output: TemporaryOutput,
    /// Command suggestions popup for auto-completion
    pub command_suggestions: CommandSuggestionsPopup,
    /// Command history for navigating previous inputs
    pub command_history: Vec<String>,
    /// Current position in command history (-1 means not navigating history)
    pub history_index: isize,
    /// Current input before history navigation began
    pub current_input: Option<String>,
}

impl TuiState {
    /// Create a new TUI state
    pub fn new(
        selected_agent_id: AgentId,
        agent_buffer: SharedBuffer,
        agent_manager: Arc<Mutex<AgentManager>>,
    ) -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            selected_agent_id,
            agent_buffer,
            should_quit: false,
            command_mode: false,
            pound_command_mode: false,
            last_interrupt_time: None,
            last_interrupt_was_process: false,
            agent_manager,
            scroll_offset: 0,
            max_scroll_offset: 0,
            visible_height: 0,
            temp_output: TemporaryOutput::new(),
            command_suggestions: CommandSuggestionsPopup::new(),
            command_history: Vec::new(),
            history_index: -1,
            current_input: None,
        }
    }

    /// Check if the current input is a command
    pub fn update_command_mode(&mut self) {
        let was_command_mode = self.command_mode;

        // Update command mode flags
        self.command_mode = self.input.starts_with('/');
        self.pound_command_mode = self.input.starts_with('#');

        // Handle command suggestions popup
        if self.command_mode {
            // If we just entered command mode or input changed, update suggestions
            if !was_command_mode || self.input.len() == 1 {
                // Just entered command mode, show suggestions
                self.command_suggestions.show(&self.input);
            } else {
                // Already in command mode, update filter
                self.command_suggestions.update_suggestions(&self.input);
            }
        } else {
            // Not in command mode, hide suggestions
            self.command_suggestions.hide();
        }
    }

    /// Update the list of agents
    /// Ensure the selected agent exists, or select the first available agent
    pub fn ensure_selected_agent_valid(&mut self) {
        if let Ok(manager) = self.agent_manager.lock() {
            // Check if the currently selected agent exists
            if manager.get_agent_handle(self.selected_agent_id).is_none() {
                // If not, select the first available agent
                if let Some((first_id, _)) = manager.get_agents().first() {
                    self.selected_agent_id = *first_id;
                    // Update buffer to the new agent
                    if let Ok(buffer) = manager.get_agent_buffer(self.selected_agent_id) {
                        self.agent_buffer = buffer;
                    }
                }
            }
        }
    }

    /// Update scroll bounds based on current content and visible area
    pub fn update_scroll(&mut self) {
        let total_lines = self.agent_buffer.lines().len();

        // Calculate new max_scroll_offset
        let new_max_scroll_offset = if total_lines > self.visible_height {
            total_lines - self.visible_height
        } else {
            0
        };

        // Check if we were already at the most recent messages (at max_scroll_offset)
        let was_at_most_recent = self.scroll_offset == self.max_scroll_offset;

        // Update the max scroll offset
        self.max_scroll_offset = new_max_scroll_offset;

        // If we were viewing the most recent messages, auto-scroll to keep showing them
        if was_at_most_recent {
            self.scroll_offset = self.max_scroll_offset;
        }
        // Otherwise just make sure we don't exceed the new maximum
        else if self.scroll_offset > self.max_scroll_offset {
            self.scroll_offset = self.max_scroll_offset;
        }
    }

    /// Scroll the conversation view
    pub fn scroll(&mut self, delta: isize) {
        let new_offset = if delta.is_negative() {
            // Scrolling up (showing older messages)
            self.scroll_offset.saturating_sub(delta.abs() as usize)
        } else {
            // Scrolling down (showing newer messages)
            self.scroll_offset.saturating_add(delta as usize)
        };

        // Clamp to valid range
        self.scroll_offset = new_offset.min(self.max_scroll_offset);
    }

    /// Scroll to the bottom of the conversation (most recent messages)
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll_offset;
    }

    /// Calculate the number of lines needed for the input text
    pub fn calculate_input_height(&self) -> u16 {
        // Use terminal size for more accurate width calculation
        // We'll set a reasonable default in case terminal size can't be determined
        let terminal_width = crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80);
        // Available width is terminal width minus borders and some padding
        let available_width = terminal_width.saturating_sub(2);

        // Count how many lines the input will take
        let input_chars = self.input.chars().count() as u16;
        if input_chars == 0 {
            return 1; // Empty input still takes one line
        }

        // Calculate lines needed (min of 1)
        let lines_needed = ((input_chars / available_width)
            + if input_chars % available_width > 0 {
                1
            } else {
                0
            })
        .max(1);

        // Count newlines in the input
        let newlines = self.input.matches('\n').count() as u16;

        // Return maximum of wrapped lines or newlines + 1
        (lines_needed).max(newlines + 1).min(10) // Cap at 10 lines maximum
    }

    /// Get a string representation of the selected agent's state
    pub fn get_agent_state_string(&self) -> String {
        if self.command_mode {
            return "Command Mode".to_string();
        }

        if self.pound_command_mode {
            return "Agent Selection Mode".to_string();
        }

        // Try to get the state from the agent manager
        if let Ok(manager) = self.agent_manager.lock() {
            if let Ok(state) = manager.get_agent_state(self.selected_agent_id) {
                return state.as_display_string();
            }
        }

        // Fallback if we can't get the state
        "Ready".to_string()
    }

    /// Get an emoji indicator for agent state
    pub fn get_state_indicator(state: &AgentState) -> &'static str {
        match state {
            AgentState::Idle => "🟢",               // Green circle for ready
            AgentState::Processing => "🤔",         // Thinking face for processing
            AgentState::RunningTool { .. } => "🔧", // Wrench for tool execution
            AgentState::Terminated => "⛔",         // No entry sign for terminated
            AgentState::Done(_) => "✅",            // Checkmark for done
        }
    }
}