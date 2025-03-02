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
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ratatui::widgets::Clear;

/// Maximum number of lines to keep in the conversation history view
const MAX_HISTORY_LINES: usize = 1000;

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
    /// Reference to the agent manager
    agent_manager: Arc<Mutex<AgentManager>>,
    /// Scroll offset for the conversation view (0 = top of conversation)
    pub scroll_offset: usize,
    /// Maximum scroll offset based on content and view size
    pub max_scroll_offset: usize,
    /// Visible content height in lines
    pub visible_height: usize,
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
            agent_manager,
            scroll_offset: 0,
            max_scroll_offset: 0,
            visible_height: 0,
        }
    }

    /// Check if the current input is a command
    pub fn update_command_mode(&mut self) {
        self.command_mode = self.input.starts_with('/');
        self.pound_command_mode = self.input.starts_with('#');
    }

    /// Add agent output to the buffer
    pub fn add_to_buffer(&mut self, text: String) {
        // Split by newlines and add each line
        for line in text.lines() {
            self.agent_buffer.stdout(line).unwrap();
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
        
        // Create the layout with header, content, and input areas
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(1),    // Content
                Constraint::Length(3), // Input
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
    }

    /// Render the header with agent list
    fn render_header(&self, f: &mut Frame, area: Rect) {
        // Get agents directly from the agent manager
        let agent_spans = if let Ok(manager) = self.agent_manager.lock() {
            let agents = manager.get_agents();
            
            // Create spans for each agent
            agents.iter()
                .map(|(id, name)| {
                    if *id == self.selected_agent_id {
                        Span::styled(
                            format!(" {} [{}] ", name, id),
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        Span::styled(
                            format!(" {} [{}] ", name, id),
                            Style::default().fg(Color::White),
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
            Style::default(),
        ));

        let header = Paragraph::new(Line::from(all_spans))
            .block(Block::default().borders(Borders::ALL).title("Agents"));

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
        let mut items: Vec<ListItem> = Vec::with_capacity(visible_height);
        
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
                    .map(|line| ListItem::new(line.converted_line.clone()))
                    .collect();
            }
        }
        
        // Fill remaining space with empty lines to ensure old content is cleared
        while items.len() < visible_height {
            items.push(ListItem::new(Line::from("")));
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
        
        let conversation = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(conversation, area);
    }

    /// Render the input area
    fn render_input(&self, f: &mut Frame, area: Rect) {
        // Create input text with styling based on mode
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
        
        // Calculate available width for input (accounting for borders)
        let available_width = area.width.saturating_sub(2) as usize;
        
        // Create a padded input string to ensure we overwrite any previous content
        // This is important when new input is shorter than old input
        let mut padded_input = self.input.clone();
        if padded_input.len() < available_width {
            // Add spaces to fill the available width
            padded_input.push_str(&" ".repeat(available_width - padded_input.len()));
        }

        let input_text = Paragraph::new(padded_input)
            .style(input_style)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: true });

        f.render_widget(input_text, area);

        // Calculate cursor position
        let cursor_x = self.cursor_position as u16 + 1; // +1 for border
        let cursor_y = area.y + 1; // +1 for border

        // Show cursor at current position
        f.set_cursor(cursor_x, cursor_y);
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
            
            // Submit on Enter
            KeyCode::Enter => {
                let input = std::mem::take(&mut self.state.input);
                self.state.cursor_position = 0;
                
                if !input.is_empty() {
                    if input.starts_with('/') {
                        self.handle_command(&input).await?;
                    } else if input.starts_with('#') {
                        self.handle_pound_command(&input).await?;
                    } else {
                        // Add user input to buffer
                        self.state.add_to_buffer(format!("> {}", input));

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
                if self.state.cursor_position > 0 {
                    self.state.input.remove(self.state.cursor_position - 1);
                    self.state.cursor_position -= 1;
                    self.state.update_command_mode();
                }
            }
            
            // Delete
            KeyCode::Delete => {
                if self.state.cursor_position < self.state.input.len() {
                    self.state.input.remove(self.state.cursor_position);
                    self.state.update_command_mode();
                }
            }
            
            // Left arrow
            KeyCode::Left => {
                if self.state.cursor_position > 0 {
                    self.state.cursor_position -= 1;
                }
            }
            
            // Right arrow
            KeyCode::Right => {
                if self.state.cursor_position < self.state.input.len() {
                    self.state.cursor_position += 1;
                }
            }
            
            // Home key handling
            KeyCode::Home => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Home: Scroll to top/oldest messages (offset = 0)
                    self.state.scroll_offset = 0;
                } else {
                    // Regular Home: Move cursor to start of input
                    self.state.cursor_position = 0;
                }
            }
            
            // End key handling
            KeyCode::End => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+End: Scroll to bottom/newest messages (offset = max)
                    self.state.scroll_to_bottom();
                } else {
                    // Regular End: Move cursor to end of input
                    self.state.cursor_position = self.state.input.len();
                }
            }
            
            // Regular character input
            KeyCode::Char(c) => {
                self.state.input.insert(self.state.cursor_position, c);
                self.state.cursor_position += 1;
                self.state.update_command_mode();
            }
            
            // Escape clears the input
            KeyCode::Esc => {
                self.state.input.clear();
                self.state.cursor_position = 0;
                self.state.command_mode = false;
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
            
            // Up/Down with shift for line-by-line scrolling
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.state.scroll(-1);
            }
            
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.state.scroll(1);
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
            if now.duration_since(last_time) < DOUBLE_PRESS_WINDOW {
                // This is a double-press, exit the application
                self.state.add_to_buffer("Received second Ctrl+C. Exiting application...".to_string());
                self.state.should_quit = true;
                return Ok(());
            }
        }
        
        // This is the first press or outside the double-press window
        self.state.last_interrupt_time = Some(now);
        
        // Get current agent state
        let agent_state = {
            let manager = self.agent_manager.lock().unwrap();
            manager.get_agent_state(self.state.selected_agent_id).ok()
        };
        
        match agent_state {
            // If running a shell command (interruptible tool) or if agent is actively processing
            Some(AgentState::RunningTool { .. }) | Some(AgentState::Processing) => {
                self.state.add_to_buffer(format!("Interrupting agent. Press Ctrl+C again within 3 seconds to exit."));
                
                // Use the dedicated interrupt channel with the agent manager
                let manager = self.agent_manager.lock().unwrap();
                manager.interrupt_agent_with_reason(
                    self.state.selected_agent_id, 
                    "User pressed Ctrl+C".to_string()
                )?;
            },
            
            // If agent is waiting for input (idle or done), just warn about second press
            _ => {
                self.state.add_to_buffer("Press Ctrl+C again within 3 seconds to exit application.".to_string());
            }
        }
        
        Ok(())
    }

    /// Handle pound command for agent switching
    async fn handle_pound_command(&mut self, cmd: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Echo command to buffer
        self.state.add_to_buffer(format!("> {}", cmd));
        
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
                    
                    self.state.add_to_buffer(format!("Switched to agent {} [{}]", agent_name, agent_id));
                } else {
                    self.state.add_to_buffer(format!("Failed to get buffer for agent {}", agent_id));
                }
            } else {
                self.state.add_to_buffer(format!("Agent with ID {} not found", agent_id));
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
                    self.state.add_to_buffer(format!("Switched to agent {} [{}]", name, agent_id));
                } else {
                    self.state.add_to_buffer(format!("Failed to get buffer for agent {}", name));
                }
            } else {
                self.state.add_to_buffer(format!("Agent '{}' not found", agent_str));
            }
        }
        
        Ok(())
    }

    /// Handle command input
    async fn handle_command(&mut self, cmd: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Echo command to buffer
        self.state.add_to_buffer(format!("> {}", cmd));
        
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "/exit" | "/quit" => {
                self.state.should_quit = true;
            }
            "/help" => {
                self.state.add_to_buffer("Available commands:".to_string());
                self.state.add_to_buffer("  /help                  - Show this help".to_string());
                self.state.add_to_buffer("  /exit, /quit           - Exit the application".to_string());
                self.state.add_to_buffer("  /interrupt             - Interrupt the current agent".to_string());
                self.state.add_to_buffer("  /model <name>          - Set the model for the current agent".to_string());
                self.state.add_to_buffer("  /tools <on|off>        - Enable or disable tools".to_string());
                self.state.add_to_buffer("  /system <text>         - Set the system prompt".to_string());
                self.state.add_to_buffer("  /reset                 - Reset the conversation".to_string());
                self.state.add_to_buffer("  #<id>                  - Switch to agent with specified ID".to_string());
                self.state.add_to_buffer("  #<name>                - Switch to agent with specified name".to_string());
                self.state.add_to_buffer("".to_string());
                self.state.add_to_buffer("Scrolling:".to_string());
                self.state.add_to_buffer("  PageUp/PageDown        - Scroll conversation by half a page".to_string());
                self.state.add_to_buffer("  Shift+Up/Down          - Scroll conversation by one line".to_string());
                self.state.add_to_buffer("  Shift+Home/End         - Jump to top/bottom of conversation".to_string());
                self.state.add_to_buffer("  Mouse wheel            - Scroll conversation".to_string());
            }
            "/interrupt" => {
                let manager = self.agent_manager.lock().unwrap();
                if let Err(e) = manager.interrupt_agent(self.state.selected_agent_id) {
                    self.state.add_to_buffer(format!("Failed to interrupt agent: {}", e));
                } else {
                    self.state.add_to_buffer(format!("Interrupt sent to agent {}", self.state.selected_agent_id));
                }
            }
            "/model" if parts.len() >= 2 => {
                let model = parts[1];
                let manager = self.agent_manager.lock().unwrap();
                if let Err(e) = manager.send_message(
                    self.state.selected_agent_id,
                    AgentMessage::Command(AgentCommand::SetModel(model.to_string())),
                ) {
                    self.state.add_to_buffer(format!("Failed to set model: {}", e));
                } else {
                    self.state.add_to_buffer(format!("Model set to {}", model));
                }
            }
            "/tools" if parts.len() >= 2 => {
                let enabled = match parts[1] {
                    "on" | "true" | "1" => true,
                    "off" | "false" | "0" => false,
                    _ => {
                        self.state.add_to_buffer(format!("Invalid value for tools: {}", parts[1]));
                        return Ok(());
                    }
                };

                let manager = self.agent_manager.lock().unwrap();
                if let Err(e) = manager.send_message(
                    self.state.selected_agent_id,
                    AgentMessage::Command(AgentCommand::EnableTools(enabled)),
                ) {
                    self.state.add_to_buffer(format!("Failed to set tools: {}", e));
                } else {
                    self.state.add_to_buffer(format!("Tools {}", if enabled { "enabled" } else { "disabled" }));
                }
            }
            "/system" if parts.len() >= 2 => {
                let prompt = &parts[1..].join(" ");
                let manager = self.agent_manager.lock().unwrap();
                if let Err(e) = manager.send_message(
                    self.state.selected_agent_id,
                    AgentMessage::Command(AgentCommand::SetSystemPrompt(prompt.to_string())),
                ) {
                    self.state.add_to_buffer(format!("Failed to set system prompt: {}", e));
                } else {
                    self.state.add_to_buffer("System prompt updated".to_string());
                }
            }
            "/reset" => {
                let manager = self.agent_manager.lock().unwrap();
                if let Err(e) = manager.send_message(
                    self.state.selected_agent_id,
                    AgentMessage::Command(AgentCommand::ResetConversation),
                ) {
                    self.state.add_to_buffer(format!("Failed to reset conversation: {}", e));
                } else {
                    self.state.add_to_buffer("Conversation reset".to_string());
                }
            }
            _ => {
                self.state.add_to_buffer(format!("Unknown command: {}", parts[0]));
            }
        }

        Ok(())
    }
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