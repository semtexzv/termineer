//! TUI interface for the AutoSWE application
//!
//! This module implements a Text User Interface (TUI) using ratatui,
//! providing a more interactive and visually appealing interface.

use crate::agent::{AgentManager, AgentCommand, AgentId, AgentMessage, AgentState};
use crate::output::SharedBuffer;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ratatui::widgets::{BorderType, Clear};

/// Maximum number of lines to keep in the conversation history view
const MAX_HISTORY_LINES: usize = 1000;

/// Temporary output window that overlays the input area and can grow upward
pub struct TemporaryOutput {
    /// Title of the output window
    pub title: String,
    /// Content lines of the output
    pub content: Vec<String>,
    /// Whether the output is visible
    pub visible: bool,
    /// Maximum number of lines to display
    pub max_lines: usize,
}

impl TemporaryOutput {
    /// Create a new temporary output
    pub fn new() -> Self {
        Self {
            title: String::new(),
            content: Vec::new(),
            visible: false,
            max_lines: 20, // Default max height
        }
    }
    
    /// Count the number of lines needed to display content
    pub fn count_lines(&self, width: u16) -> usize {
        self.content.iter().map(|line| {
            // Calculate how many display lines this content line will take
            // with wrapping at the specified width
            let chars = line.chars().count();
            if chars == 0 {
                1 // Empty line still takes one line
            } else {
                // Number of full lines plus one for any partial line
                (chars / width as usize) + if chars % width as usize > 0 { 1 } else { 0 }
            }
        }).sum()
    }

    /// Hide the output
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Show output with new content
    pub fn show(&mut self, title: String, content: String) {
        self.title = title;
        self.content = content.lines().map(String::from).collect();
        self.visible = true;
    }
}

/// Command suggestion entry
#[derive(Clone, Debug)]
pub struct CommandSuggestion {
    /// Command name (including the slash)
    pub name: String,
    /// Command description
    pub description: String,
}

/// Command suggestions popup for auto-completion
pub struct CommandSuggestionsPopup {
    /// List of all available commands
    pub all_commands: Vec<CommandSuggestion>,
    /// Filtered commands matching the current input
    pub filtered_commands: Vec<CommandSuggestion>,
    /// Currently selected command index
    pub selected_index: usize,
    /// Whether the popup is visible
    pub visible: bool,
}

impl CommandSuggestionsPopup {
    /// Create a new command suggestions popup
    pub fn new() -> Self {
        // Initialize with all available commands
        let all_commands = vec![
            CommandSuggestion { name: "/help".to_string(), description: "Show available commands".to_string() },
            CommandSuggestion { name: "/exit".to_string(), description: "Exit the application".to_string() },
            CommandSuggestion { name: "/quit".to_string(), description: "Exit the application".to_string() },
            CommandSuggestion { name: "/interrupt".to_string(), description: "Interrupt the current agent".to_string() },
            CommandSuggestion { name: "/model".to_string(), description: "Set the model for the current agent".to_string() },
            CommandSuggestion { name: "/tools".to_string(), description: "Enable or disable tools".to_string() },
            CommandSuggestion { name: "/system".to_string(), description: "Set the system prompt".to_string() },
            CommandSuggestion { name: "/reset".to_string(), description: "Reset the conversation".to_string() },
        ];
        
        Self {
            all_commands: all_commands.clone(),
            filtered_commands: all_commands,
            selected_index: 0,
            visible: false,
        }
    }
    
    /// Show the suggestions popup and filter based on current input
    pub fn show(&mut self, current_input: &str) {
        self.visible = true;
        self.update_suggestions(current_input);
    }
    
    /// Hide the suggestions popup
    pub fn hide(&mut self) {
        self.visible = false;
    }
    
    /// Update filtered suggestions based on current input
    pub fn update_suggestions(&mut self, current_input: &str) {
        // Skip the leading slash for matching
        let search_text = current_input.trim_start_matches('/');
        
        // If empty, show all commands
        if search_text.is_empty() {
            self.filtered_commands = self.all_commands.clone();
            self.selected_index = 0;
            return;
        }
        
        // Filter commands that match the input prefix
        self.filtered_commands = self.all_commands
            .iter()
            .filter(|cmd| {
                cmd.name.trim_start_matches('/').starts_with(search_text)
            })
            .cloned()
            .collect();
        
        // Reset selection index
        self.selected_index = 0;
    }
    
    /// Select the next suggestion
    pub fn next(&mut self) {
        if !self.filtered_commands.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_commands.len();
        }
    }
    
    /// Get the currently selected command if any
    pub fn selected_command(&self) -> Option<&CommandSuggestion> {
        if self.filtered_commands.is_empty() {
            None
        } else {
            self.filtered_commands.get(self.selected_index)
        }
    }
}

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
    pub last_interrupt_time: Option<std::time::Instant>,
    /// Whether the last Ctrl+C was used to interrupt an agent/shell
    pub last_interrupt_was_process: bool,
    /// Reference to the agent manager
    agent_manager: Arc<Mutex<AgentManager>>,
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
    pub fn new(selected_agent_id: AgentId, agent_buffer: SharedBuffer, agent_manager: Arc<Mutex<AgentManager>>) -> Self {
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

    /// Draw the UI components
    fn ui(&self, f: &mut Frame) {
        let size = f.size();
        f.render_widget(Clear, size);
        
        // Calculate the height needed for input box based on content
        let input_height = if self.temp_output.visible {
            3 // Default height when showing temporary output
        } else {
            // Dynamic height based on input content (min 3, includes borders)
            self.calculate_input_height() + 2 // +2 for borders
        };
        
        // Create the layout with header, content, and variable-height input areas
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),                 // Header
                Constraint::Min(1),                    // Content (flexible)
                Constraint::Length(input_height),      // Dynamic-height input
            ])
            .split(size);

        // Render the header with agent list
        f.render_widget(Clear, chunks[0]);
        self.render_header(f, chunks[0]);

        // Render the content area with conversation history
        f.render_widget(Clear, chunks[1]);
        self.render_content(f, chunks[1]);

        // Render the input prompt
        f.render_widget(Clear, chunks[2]);
        self.render_input(f, chunks[2]);
        
        // Render the command suggestions popup if in command mode and temp output is not visible
        if self.command_mode && !self.temp_output.visible {
            self.render_command_suggestions(f);
        }
        
        // Render the temporary output window if visible
        if self.temp_output.visible {
            self.render_temp_output(f, chunks[2], chunks[1]);
        }
    }
    
    /// Render the temporary output window that overlays input and grows upward
    fn render_temp_output(&self, f: &mut Frame, input_area: Rect, content_area: Rect) {
        // Start with the input area as the base
        let mut output_area = input_area;
        
        // Calculate the total number of lines needed for content
        let available_width = output_area.width.saturating_sub(4); // Allow for borders and padding
        let needed_lines = self.temp_output.count_lines(available_width);
        
        // Determine how many lines we can extend upward into the content area
        let max_extension = content_area.height.saturating_sub(5) as usize; // Leave 5 lines of content visible
        let extension_lines = needed_lines.saturating_sub(1).min(max_extension);
        
        // Extend upward if needed
        if extension_lines > 0 {
            output_area.y = output_area.y.saturating_sub(extension_lines as u16);
            output_area.height += extension_lines as u16;
        }
        
        // Clear the area
        f.render_widget(Clear, output_area);
        
        // Create the temporary output widget with dark orange styling
        let content_text = self.temp_output.content.join("\n");
        let output_widget = Paragraph::new(content_text)
            .style(Style::default()
                .fg(Color::LightCyan) // More visible cyan text instead of white
                .bg(Color::Rgb(180, 80, 0))) // Dark orange background
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(255, 140, 0))) // Brighter orange border
                .title(format!("{} (Press ESC or Enter to dismiss)", self.temp_output.title))
                .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
            .wrap(Wrap { trim: true });
        
        // Render the output
        f.render_widget(output_widget, output_area);
    }
    
    /// Render command suggestions popup
    fn render_command_suggestions(&self, f: &mut Frame) {
        // Only render if suggestions are visible and we have any
        if !self.command_suggestions.visible || self.command_suggestions.filtered_commands.is_empty() {
            return;
        }
        
        let area = f.size();
        
        // Calculate total rows needed (one per command)
        let num_commands = self.command_suggestions.filtered_commands.len();
        
        // Set a maximum height for the popup
        let popup_height = num_commands.min(8) as u16 + 2; // +2 for borders
        
        // Calculate width based on longest command and description
        let max_cmd_width = self.command_suggestions.filtered_commands
            .iter()
            .map(|cmd| cmd.name.len())
            .max()
            .unwrap_or(10) as u16;
            
        let max_desc_width = self.command_suggestions.filtered_commands
            .iter()
            .map(|cmd| cmd.description.len())
            .max()
            .unwrap_or(30) as u16;
        
        // Set popup width with some padding
        let popup_width = (max_cmd_width + max_desc_width + 10).min(area.width.saturating_sub(4)).max(30);
        
        // Position popup at the left bottom edge of screen, above input area
        let input_area_y = area.height.saturating_sub(3); // Input is 3 lines from bottom
        
        // Fixed position at left edge
        let popup_x = 0;
        let popup_y = input_area_y.saturating_sub(popup_height);
        
        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        
        // Clear the area under the popup
        f.render_widget(Clear, popup_area);
        
        // Create lines for each suggestion with proper highlighting
        let mut content_lines: Vec<Line> = Vec::with_capacity(num_commands);
        
        for (index, suggestion) in self.command_suggestions.filtered_commands.iter().enumerate() {
            // Determine if this is the selected suggestion
            let is_selected = index == self.command_suggestions.selected_index;
            
            // Create style for command name based on selection
            let cmd_style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            
            // Create style for description
            let desc_style = if is_selected {
                Style::default().bg(Color::White).fg(Color::Black)
            } else {
                Style::default().fg(Color::Gray)
            };
            
            // Format the line with proper spacing
            let line = Line::from(vec![
                Span::styled(suggestion.name.clone(), cmd_style),
                Span::styled(" - ", desc_style),
                Span::styled(suggestion.description.clone(), desc_style),
            ]);
            
            content_lines.push(line);
        }
        
        // Create the suggestions widget
        let suggestions_widget = Paragraph::new(content_lines)
            .block(Block::default()
                .borders(Borders::ALL)
                .title("Commands (TAB to complete)"));
        
        // Render the suggestions
        f.render_widget(suggestions_widget, popup_area);
    }

    /// Get an emoji indicator for agent state
    fn get_state_indicator(state: &AgentState) -> &'static str {
        match state {
            AgentState::Idle => "🟢", // Green circle for ready
            AgentState::Processing => "🤔", // Thinking face for processing
            AgentState::RunningTool { .. } => "🔧", // Wrench for tool execution
            AgentState::Terminated => "⛔", // No entry sign for terminated
            AgentState::Done => "✅", // Checkmark for done
        }
    }

    /// Render the header with agent list
    fn render_header(&self, f: &mut Frame, area: Rect) {
        // Get agents directly from the agent manager
        let agent_spans = if let Ok(manager) = self.agent_manager.lock() {
            let agents = manager.get_agents();
            
            // Collect all agent states in a single lock operation
            let mut agent_states = Vec::new();
            for (id, _) in &agents {
                let state = manager.get_agent_state(*id).ok();
                agent_states.push(state);
            }
            
            // Create spans for each agent
            agents.iter().zip(agent_states.iter())
                .map(|((id, name), state_opt)| {
                    // Get state indicator based on agent state
                    let state_char = if let Some(state) = state_opt {
                        Self::get_state_indicator(state)
                    } else {
                        "?" // Unknown state
                    };
                    
                    if *id == self.selected_agent_id {
                        Span::styled(
                            format!(" {} {} [{}] ", state_char, name, id),
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(
                            format!(" {} {} [{}] ", state_char, name, id),
                            Style::default().fg(Color::LightBlue),
                        )
                    }
                })
                .collect::<Vec<Span>>()
        } else {
            Vec::new()
        };

        // Add a final span with empty content to fill remaining space
        // This ensures old content is fully cleared
        let mut all_spans = agent_spans;
        all_spans.push(Span::styled(
            " ".repeat((area.width as usize).saturating_sub(2)), // -2 for borders
            Style::default().fg(Color::DarkGray), // Ensure text is visible but subdued
        ));

        let header = Paragraph::new(Line::from(all_spans))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Agents"));

        f.render_widget(header, area);
    }

    /// Render the content area with conversation history
    fn render_content(&self, f: &mut Frame, area: Rect) {
        let lines = self.agent_buffer.lines();
        let total_lines = lines.len();
        
        // Calculate visible area height (accounting for borders)
        // -2 for the top and bottom borders of the block
        let visible_height = area.height.saturating_sub(2) as usize;
        
        // Create empty list items for filling the visible area
        let mut items: Vec<Line> = Vec::with_capacity(visible_height);
        
        if total_lines > 0 {
            // Calculate the start index for the visible region
            let start_idx = if self.scroll_offset < total_lines {
                self.scroll_offset
            } else {
                0
            };
            
            // When at maximum scroll offset (bottom), we want to ensure the last line is visible
            // This requires special handling
            let adjusted_start = if self.scroll_offset == self.max_scroll_offset && total_lines > visible_height {
                // Ensure we show the last line by adjusting start index
                // This forces display of the range ending with the last line
                total_lines - visible_height
            } else {
                // Normal scroll position
                start_idx
            };
            
            // Get the visible range of lines
            let end_idx = (adjusted_start + visible_height).min(total_lines);
            
            // Extract the lines for the visible range
            if adjusted_start < total_lines {
                // Use an iterator to be more explicit about the range
                items = (adjusted_start..end_idx)
                    .filter_map(|i| lines.get(i))
                    .map(|line| line.converted_line.clone())
                    .collect();
            }
        }
        
        // Fill remaining space with empty lines to ensure old content is cleared
        while items.len() < visible_height {
            items.push(Line::from(""));
        }

        // Create title with scroll info and most recent messages indicator
        let scroll_info = if total_lines > visible_height {
            let latest_indicator = if self.scroll_offset == self.max_scroll_offset {
                " (Most Recent ↓)"
            } else {
                ""
            };
            
            format!(" | Scroll: {}/{}{}", self.scroll_offset, self.max_scroll_offset, latest_indicator)
        } else {
            String::new()
        };
        
        let title = format!("Conversation ({} lines{})", total_lines, scroll_info);
        
        let conversation = Paragraph::new(items)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(title));
        f.render_widget(conversation, area);
    }

    /// Calculate the number of lines needed for the input text
    fn calculate_input_height(&self) -> u16 {
        // Calculate required height based on input text and width
        // Get a reasonable estimate of available width (approximate terminal width minus borders and padding)
        let estimated_width = 80u16.saturating_sub(4); // Reasonable default with borders
        
        // Count how many lines the input will take
        let input_chars = self.input.chars().count() as u16;
        if input_chars == 0 {
            return 1; // Empty input still takes one line
        }
        
        // Calculate lines needed (min of 1)
        let lines_needed = ((input_chars / estimated_width) + if input_chars % estimated_width > 0 { 1 } else { 0 })
            .max(1);
            
        // Count newlines in the input
        let newlines = self.input.matches('\n').count() as u16;
        
        // Return maximum of wrapped lines or newlines + 1
        (lines_needed).max(newlines + 1).min(10) // Cap at 10 lines maximum
    }
    
    /// Render the input area with support for multi-line text
    fn render_input(&self, f: &mut Frame, area: Rect) {
        // Normal input rendering
        let input_style = if self.command_mode {
            Style::default().fg(Color::Yellow)
        } else if self.pound_command_mode {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Black)
        };

        // Get the agent state from the agent manager
        let agent_state_str = self.get_agent_state_string();
        
        // Get the current agent name directly from the agent manager
        let agent_name = if let Ok(manager) = self.agent_manager.lock() {
            // Clone the name to avoid lifetime issues
            manager.get_agent_handle(self.selected_agent_id)
                .map(|h| h.name.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        };
        
        // Create title with agent state
        let title = format!("Input [{} [{}] | {}]", agent_name, self.selected_agent_id, agent_state_str);

        // Create the input widget with text wrapping enabled
        let input_text = Paragraph::new(self.input.clone())
            .style(input_style)
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(title))
            .wrap(Wrap { trim: true }); // Enable wrapping, don't trim to preserve formatting

        f.render_widget(input_text, area);

        // Only show cursor if temporary output is not visible
        if !self.temp_output.visible {
            // Calculate cursor position for wrapped text
            // This is a simplified calculation that works for basic wrapping
            let available_width = area.width.saturating_sub(2) as usize; // -2 for borders
            
            // Calculate cursor row and column
            let cursor_pos_in_chars = self.cursor_position;
            let cursor_column = (cursor_pos_in_chars % available_width) as u16 + 1; // +1 for border
            let cursor_row = (cursor_pos_in_chars / available_width) as u16 + 1; // +1 for border
            
            // Show cursor at calculated position
            f.set_cursor(
                area.x + cursor_column,
                area.y + cursor_row
            );
        }
    }
    
    /// Get a string representation of the selected agent's state
    fn get_agent_state_string(&self) -> String {
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
}

/// TUI interface for the AutoSWE application
pub struct TuiInterface {
    /// Terminal instance for the TUI
    terminal: Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    /// Application state
    state: TuiState,
    /// Agent manager
    agent_manager: Arc<Mutex<AgentManager>>,
}

impl TuiInterface {
    /// Create a new TUI interface
    pub fn new(
        agent_manager: Arc<Mutex<AgentManager>>,
        main_agent_id: AgentId,
    ) -> Result<Self, io::Error> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let buffer = agent_manager.lock().unwrap().get_agent_buffer(main_agent_id).unwrap();
        let state = TuiState::new(main_agent_id, buffer, agent_manager.clone());

        Ok(Self {
            terminal,
            state,
            agent_manager,
        })
    }

    /// Run the TUI interface
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        while !self.state.should_quit {
            // Process all pending events first
            let mut events_processed = false;
            
            // Process events in a batch until there are none left
            while event::poll(Duration::from_millis(0))? {
                events_processed = true;
                match event::read()? {
                    Event::Key(key) => {
                        self.handle_key_event(key).await?;
                    },
                    Event::Mouse(mouse) => {
                        self.handle_mouse_event(mouse).await?;
                    },
                    Event::Resize(_, _) => {
                        // Terminal resize - bounds will be updated in draw
                    },
                    _ => {}
                }
                
                // Exit early if quit flag was set by an event handler
                if self.state.should_quit {
                    break;
                }
            }
            
            // Ensure we have a valid agent selected before drawing
            self.state.ensure_selected_agent_valid();
            
            // Draw the UI after processing all pending events
            self.terminal.draw(|f| {
                // Update visible height based on frame size
                let content_height = f.size().height.saturating_sub(6) as usize; // Account for headers and borders
                self.state.visible_height = content_height;
                self.state.update_scroll();
                self.state.ui(f);
            })?;
            
            // If no events were processed, wait a bit to avoid busy-waiting
            if !events_processed {
                event::poll(Duration::from_millis(16))?;
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            event::DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;

        Ok(())
    }

    /// Handle key events
    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match key.code {
            // Multi-level interrupt with Ctrl+C
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_ctrl_c_interrupt().await?;
            }
            
            // Submit on Enter or insert newline with Shift+Enter
            KeyCode::Enter => {
                // If temporary output is visible, dismiss it, reset state, and return
                if self.state.temp_output.visible {
                    self.state.temp_output.hide();
                    // Clear input and hide suggestions when dismissing output
                    self.state.input.clear();
                    self.state.cursor_position = 0;
                    self.state.command_mode = false;
                    self.state.command_suggestions.hide();
                    return Ok(());
                }
                
                // If Shift is held, insert a newline instead of submitting
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.state.input.insert(self.state.cursor_position, '\n');
                    self.state.cursor_position += 1;
                    return Ok(());
                }
                
                let input = std::mem::take(&mut self.state.input);
                self.state.cursor_position = 0;
                
                // Reset history navigation state
                self.state.history_index = -1;
                self.state.current_input = None;
                
                if !input.is_empty() {
                    // Add to command history (if not a duplicate of the last command)
                    if self.state.command_history.last() != Some(&input) {
                        // Add to history, limiting size to 100 entries
                        self.state.command_history.push(input.clone());
                        if self.state.command_history.len() > 100 {
                            self.state.command_history.remove(0);
                        }
                    }
                    
                    if input.starts_with('/') {
                        // Simply send the command directly to the agent
                        // Add user input to buffer with blue color (slash command)
                        self.state.agent_buffer.stdout(&format!("Command: {}", input)).unwrap();

                        // Send to selected agent
                        let manager = self.agent_manager.lock().unwrap();
                        manager.send_message(
                            self.state.selected_agent_id,
                            AgentMessage::UserInput(input),
                        )?;
                        
                        // Clear the input after submitting
                        self.state.input.clear();
                        self.state.cursor_position = 0;
                        self.state.command_mode = false;
                    } else if input.starts_with('#') {
                        // For pound commands for agent switching, keep the special handling
                        self.handle_pound_command(&input).await?;
                        
                        // Clear the input after submitting
                        self.state.input.clear();
                        self.state.cursor_position = 0;
                        self.state.pound_command_mode = false;
                    } else {
                        // Don't add user input to buffer here, agent will handle it
                        // No need to prefix with chevron as the agent will format it properly

                        // Send to selected agent
                        let manager = self.agent_manager.lock().unwrap();
                        manager.send_message(
                            self.state.selected_agent_id,
                            AgentMessage::UserInput(input),
                        )?;
                    }
                }
            }
            
            // Backspace
            KeyCode::Backspace => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                if self.state.cursor_position > 0 {
                    self.state.input.remove(self.state.cursor_position - 1);
                    self.state.cursor_position -= 1;
                    self.state.update_command_mode();
                    
                    // Handle special case: check if we're still in command mode
                    if self.state.command_mode {
                        self.state.command_suggestions.update_suggestions(&self.state.input);
                    }
                }
            }
            
            // Delete
            KeyCode::Delete => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                if self.state.cursor_position < self.state.input.len() {
                    self.state.input.remove(self.state.cursor_position);
                    self.state.update_command_mode();
                    
                    // Update command suggestions if still in command mode
                    if self.state.command_mode {
                        self.state.command_suggestions.update_suggestions(&self.state.input);
                    }
                }
            }
            
            // Left arrow (with modifiers for macOS conventions)
            KeyCode::Left => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                // Command + Left: Move to beginning of line (macOS convention)
                if key.modifiers.contains(KeyModifiers::META) {
                    self.state.cursor_position = 0;
                }
                // Option/Alt + Left: Move one word left (macOS convention)
                else if key.modifiers.contains(KeyModifiers::ALT) {
                    // Find previous word boundary
                    if self.state.cursor_position > 0 {
                        // First skip any spaces directly to the left
                        let mut pos = self.state.cursor_position;
                        let chars: Vec<char> = self.state.input.chars().collect();
                        
                        // Skip spaces backward
                        while pos > 0 && chars[pos - 1].is_whitespace() {
                            pos -= 1;
                        }
                        
                        // Then skip non-spaces backward (the word)
                        while pos > 0 && !chars[pos - 1].is_whitespace() {
                            pos -= 1;
                        }
                        
                        self.state.cursor_position = pos;
                    }
                }
                // Regular left arrow: Move one character left
                else if self.state.cursor_position > 0 {
                    self.state.cursor_position -= 1;
                }
            }
            
            // Right arrow (with modifiers for macOS conventions)
            KeyCode::Right => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                // Command + Right: Move to end of line (macOS convention)
                if key.modifiers.contains(KeyModifiers::META) {
                    self.state.cursor_position = self.state.input.len();
                }
                // Option/Alt + Right: Move one word right (macOS convention)
                else if key.modifiers.contains(KeyModifiers::ALT) {
                    // Find next word boundary
                    if self.state.cursor_position < self.state.input.len() {
                        let mut pos = self.state.cursor_position;
                        let chars: Vec<char> = self.state.input.chars().collect();
                        
                        // Skip non-spaces forward (current word)
                        while pos < chars.len() && !chars[pos].is_whitespace() {
                            pos += 1;
                        }
                        
                        // Then skip spaces forward
                        while pos < chars.len() && chars[pos].is_whitespace() {
                            pos += 1;
                        }
                        
                        self.state.cursor_position = pos;
                    }
                } 
                // Regular right arrow: Move one character right
                else if self.state.cursor_position < self.state.input.len() {
                    self.state.cursor_position += 1;
                }
            }
            
            // Home key handling
            KeyCode::Home => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Home: Scroll to top/oldest messages (offset = 0)
                    self.state.scroll_offset = 0;
                } else if !self.state.temp_output.visible {
                    // Regular Home: Move cursor to start of input
                    // Only if temporary output is not visible
                    self.state.cursor_position = 0;
                }
            }
            
            // End key handling
            KeyCode::End => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+End: Scroll to bottom/newest messages (offset = max)
                    self.state.scroll_to_bottom();
                } else if !self.state.temp_output.visible {
                    // Regular End: Move cursor to end of input
                    // Only if temporary output is not visible
                    self.state.cursor_position = self.state.input.len();
                }
            }
            
            // Regular character input
            KeyCode::Char(c) => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                // Handle Option+Right (commonly produces 'f' character in macOS terminal - "forward")
                if c == 'f' && key.modifiers.contains(KeyModifiers::ALT) {
                    // Move one word right
                    if self.state.cursor_position < self.state.input.len() {
                        let mut pos = self.state.cursor_position;
                        let chars: Vec<char> = self.state.input.chars().collect();
                        
                        // Skip non-spaces forward (current word)
                        while pos < chars.len() && !chars[pos].is_whitespace() {
                            pos += 1;
                        }
                        
                        // Then skip spaces forward
                        while pos < chars.len() && chars[pos].is_whitespace() {
                            pos += 1;
                        }
                        
                        self.state.cursor_position = pos;
                    }
                    return Ok(());
                }
                
                // Handle Option+Left (commonly produces 'b' character in macOS terminal - "backward")
                if c == 'b' && key.modifiers.contains(KeyModifiers::ALT) {
                    // Move one word left
                    if self.state.cursor_position > 0 {
                        // First skip any spaces directly to the left
                        let mut pos = self.state.cursor_position;
                        let chars: Vec<char> = self.state.input.chars().collect();
                        
                        // Skip spaces backward
                        while pos > 0 && chars[pos - 1].is_whitespace() {
                            pos -= 1;
                        }
                        
                        // Then skip non-spaces backward (the word)
                        while pos > 0 && !chars[pos - 1].is_whitespace() {
                            pos -= 1;
                        }
                        
                        self.state.cursor_position = pos;
                    }
                    return Ok(());
                }
                
                self.state.input.insert(self.state.cursor_position, c);
                self.state.cursor_position += 1;
                self.state.update_command_mode();
                
                // Special handling when starting a command - show suggestions immediately
                if self.state.input == "/" {
                    self.state.command_suggestions.show("/");
                }
            }
            
            // Tab key for command completion
            KeyCode::Tab => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                // Only handle Tab in command mode with visible suggestions
                if self.state.command_mode && self.state.command_suggestions.visible {
                    // Get the currently selected command
                    if let Some(selected) = self.state.command_suggestions.selected_command() {
                        // Replace current input with the selected command
                        self.state.input = selected.name.clone();
                        self.state.cursor_position = self.state.input.len();
                        
                        // If there's only one suggestion, add a space for parameters
                        if self.state.command_suggestions.filtered_commands.len() == 1 {
                            self.state.input.push(' ');
                            self.state.cursor_position += 1;
                            // Hide suggestions after completion
                            self.state.command_suggestions.hide();
                        } else {
                            // More than one suggestion, cycle to next
                            self.state.command_suggestions.next();
                        }
                    }
                }
            }
            
            // Escape either dismisses temp output, hides suggestions, or clears the input
            KeyCode::Esc => {
                if self.state.temp_output.visible {
                    self.state.temp_output.hide();
                } else if self.state.command_suggestions.visible {
                    self.state.command_suggestions.hide();
                } else {
                    // Clear input and reset history navigation
                    self.state.input.clear();
                    self.state.cursor_position = 0;
                    self.state.command_mode = false;
                    self.state.history_index = -1;
                    self.state.current_input = None;
                }
            }
            
            // PageUp/PageDown for scrolling
            KeyCode::PageUp => {
                // Scroll up (showing older messages)
                let scroll_amount = self.state.visible_height / 2;
                self.state.scroll(-(scroll_amount as isize));
            }
            
            KeyCode::PageDown => {
                // Scroll down (showing newer messages)
                let scroll_amount = self.state.visible_height / 2;
                self.state.scroll(scroll_amount as isize);
            }
            
            // Up arrow handling - navigate suggestions, history, or scroll
            KeyCode::Up => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                // If command suggestions are visible, navigate up through them
                if self.state.command_mode && self.state.command_suggestions.visible && !self.state.command_suggestions.filtered_commands.is_empty() {
                    // Navigate to previous suggestion (looping to bottom if at top)
                    let current = self.state.command_suggestions.selected_index;
                    let count = self.state.command_suggestions.filtered_commands.len();
                    
                    // Calculate previous index with wrap-around
                    let prev = if current == 0 { count - 1 } else { current - 1 };
                    self.state.command_suggestions.selected_index = prev;
                    
                    // Automatically update input with the currently selected suggestion
                    if let Some(selected) = self.state.command_suggestions.selected_command() {
                        self.state.input = selected.name.clone();
                        self.state.cursor_position = self.state.input.len();
                    }
                } 
                // Handle as scroll with shift modifier
                else if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.state.scroll(-1);
                }
                // If in normal input mode, navigate command history
                else if !self.state.command_history.is_empty() {
                    // Save current input when starting history navigation
                    if self.state.history_index == -1 {
                        self.state.current_input = Some(self.state.input.clone());
                    }
                    
                    // Go backward in history if not at beginning
                    if self.state.history_index < (self.state.command_history.len() as isize - 1) {
                        self.state.history_index += 1;
                        let history_entry = &self.state.command_history[self.state.command_history.len() - 1 - self.state.history_index as usize];
                        self.state.input = history_entry.clone();
                        self.state.cursor_position = self.state.input.len();
                    }
                }
            }
            
            // Down arrow handling - navigate suggestions, history, or scroll
            KeyCode::Down => {
                // Ignore if temporary output is visible
                if self.state.temp_output.visible {
                    return Ok(());
                }
                
                // If command suggestions are visible, navigate down through them
                if self.state.command_mode && self.state.command_suggestions.visible && !self.state.command_suggestions.filtered_commands.is_empty() {
                    // Navigate to next suggestion (looping to top if at bottom)
                    self.state.command_suggestions.next();
                    
                    // Automatically update input with the currently selected suggestion
                    if let Some(selected) = self.state.command_suggestions.selected_command() {
                        self.state.input = selected.name.clone();
                        self.state.cursor_position = self.state.input.len();
                    }
                } 
                // Handle as scroll with shift modifier
                else if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.state.scroll(1);
                }
                // If currently navigating history, go forward
                else if self.state.history_index > -1 {
                    self.state.history_index -= 1;
                    
                    // If reached beyond the most recent history item, restore the original input
                    if self.state.history_index == -1 {
                        if let Some(original_input) = self.state.current_input.take() {
                            self.state.input = original_input;
                        } else {
                            self.state.input.clear();
                        }
                    } else {
                        // Otherwise show the history entry
                        let history_entry = &self.state.command_history[self.state.command_history.len() - 1 - self.state.history_index as usize];
                        self.state.input = history_entry.clone();
                    }
                    self.state.cursor_position = self.state.input.len();
                }
            }
            
            // Ignore other keys
            _ => {}
        }
        
        Ok(())
    }

    /// Handle mouse events (simplified version)
    async fn handle_mouse_event(&mut self, mouse: event::MouseEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Simple mouse wheel scrolling implementation
        match mouse.kind {
            event::MouseEventKind::ScrollDown => {
                // Scroll down (increase offset to show newer/more recent messages)
                self.state.scroll(3);
            },
            event::MouseEventKind::ScrollUp => {
                // Scroll up (decrease offset to show older messages)
                self.state.scroll(-3);
            },
            _ => {}
        }
        
        Ok(())
    }

    /// Handle Ctrl+C interrupt with multi-level behavior
    async fn handle_ctrl_c_interrupt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Define the double-press window (3 seconds)
        const DOUBLE_PRESS_WINDOW: std::time::Duration = std::time::Duration::from_secs(3);
        
        // Get current time
        let now = std::time::Instant::now();
        
        // Check if this is a double-press (second Ctrl+C within window)
        if let Some(last_time) = self.state.last_interrupt_time {
            // Only count as double-press if previous Ctrl+C wasn't for interrupting a process
            if !self.state.last_interrupt_was_process && now.duration_since(last_time) < DOUBLE_PRESS_WINDOW {
                // This is a double-press, exit the application
                let popup_title = "Exiting Application".to_string();
                let popup_content = "Received second Ctrl+C. Exiting application...".to_string();
                self.show_command_result(popup_title, popup_content);
                
                self.state.should_quit = true;
                return Ok(());
            }
        }
        
        // Get current agent state
        let agent_state = {
            let manager = self.agent_manager.lock().unwrap();
            manager.get_agent_state(self.state.selected_agent_id).ok()
        };
        
        let popup_title = "Interrupt".to_string();
        let popup_content;
        
        match agent_state {
            // If running a shell command (interruptible tool) or if agent is actively processing
            Some(AgentState::RunningTool { .. }) | Some(AgentState::Processing) => {
                // Use the dedicated interrupt channel with the agent manager
                let manager = self.agent_manager.lock().unwrap();
                manager.interrupt_agent_with_reason(
                    self.state.selected_agent_id, 
                    "User pressed Ctrl+C".to_string()
                )?;
                
                // Mark that we used Ctrl+C to interrupt a process
                // This prevents it from counting towards the double-press exit timer
                self.state.last_interrupt_time = Some(now);
                self.state.last_interrupt_was_process = true;
                
                // Don't show popup when interrupting an agent
                // Just silently interrupt the process
            },
            
            // If agent is waiting for input (idle or done), start the double-press timer
            _ => {
                popup_content = "Press Ctrl+C again within 3 seconds to exit application.".to_string();
                
                // Start the double-press timer for exiting the application
                self.state.last_interrupt_time = Some(now);
                self.state.last_interrupt_was_process = false;
                
                // Only show popup when we're counting toward application exit
                self.show_command_result(popup_title, popup_content);
            }
        }
        
        Ok(())
    }

    /// This function is kept as a no-op for compatibility
    /// Command output is now sent directly to the agent
    fn show_command_result(&mut self, _title: String, _content: String) {
        // No-op - command output now goes to agent
    }
    
    /// Handle pound command for agent switching
    async fn handle_pound_command(&mut self, cmd: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create popup for command result
        let command_title = format!("Agent Selection: {}", cmd);
        let mut result = String::new();
        
        // Parse the agent number from the command
        let agent_str = cmd.trim_start_matches('#').trim();
        
        // Try to parse as a number first (for ID-based selection)
        if let Ok(agent_id) = agent_str.parse::<u64>() {
            let agent_id = AgentId(agent_id);
            
            // Check if this agent exists
            let agent_exists = self.agent_manager.lock()
                .map(|manager| manager.get_agent_handle(agent_id).is_some())
                .unwrap_or(false);
            
            if agent_exists {
                // Switch to this agent
                self.state.selected_agent_id = agent_id;
                
                // Update buffer to show the selected agent's output
                let manager = self.agent_manager.lock().unwrap();
                if let Ok(buffer) = manager.get_agent_buffer(agent_id) {
                    self.state.agent_buffer = buffer;
                    
                    // Get agent name from manager
                    let agent_name = manager.get_agent_handle(agent_id)
                        .map(|handle| handle.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());
                    
                    result.push_str(&format!("Switched to agent {} [{}]", agent_name, agent_id));
                } else {
                    result.push_str(&format!("Failed to get buffer for agent {}", agent_id));
                }
            } else {
                result.push_str(&format!("Agent with ID {} not found", agent_id));
            }
        } else {
            // Try to find agent by name using the manager
            let manager_result = self.agent_manager.lock().ok();
            
            let agent_info = manager_result.and_then(|manager| {
                manager.get_agent_id_by_name(agent_str).map(|id| {
                    let name = manager.get_agent_handle(id)
                        .map(|h| h.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());
                    (id, name)
                })
            });
                
            if let Some((agent_id, name)) = agent_info {
                // Switch to this agent
                self.state.selected_agent_id = agent_id;
                
                // Update buffer to show the selected agent's output
                let manager = self.agent_manager.lock().unwrap();
                if let Ok(buffer) = manager.get_agent_buffer(agent_id) {
                    self.state.agent_buffer = buffer;
                    result.push_str(&format!("Switched to agent {} [{}]", name, agent_id));
                } else {
                    result.push_str(&format!("Failed to get buffer for agent {}", name));
                }
            } else {
                result.push_str(&format!("Agent '{}' not found", agent_str));
            }
        }
        
        // Show result in popup
        self.show_command_result(command_title, result);
        
        Ok(())
    }

    // handle_command method removed as commands are now sent directly to the agent
}

impl Drop for TuiInterface {
    fn drop(&mut self) {
        // Ensure terminal is properly cleaned up
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            event::DisableMouseCapture
        );
    }
}