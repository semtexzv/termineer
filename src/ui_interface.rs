//! TUI interface for the AutoSWE application
//!
//! This module implements a Text User Interface (TUI) using ratatui,
//! providing a more interactive and visually appealing interface.

use crate::agent::{AgentManager, AgentCommand, AgentId, AgentMessage};
use crate::output::SharedBuffer;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    /// List of all agent IDs
    pub agent_ids: Vec<AgentId>,
    /// Buffer for the selected agent's output.
    pub agent_buffer: SharedBuffer,
    /// Whether the application should exit
    pub should_quit: bool,
    /// Command mode indicator (when input starts with '/')
    pub command_mode: bool,
}

impl TuiState {
    /// Create a new TUI state
    pub fn new(selected_agent_id: AgentId, agent_buffer: SharedBuffer) -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            selected_agent_id,
            agent_ids: vec![selected_agent_id],
            agent_buffer,
            should_quit: false,
            command_mode: false,
        }
    }

    /// Check if the current input is a command
    pub fn update_command_mode(&mut self) {
        self.command_mode = self.input.starts_with('/');
    }

    /// Add agent output to the buffer
    pub fn add_to_buffer(&mut self, text: String) {
        // Split by newlines and add each line
        for line in text.lines() {
            self.agent_buffer.stdout(line).unwrap();
        }
    }

    /// Update the list of agents
    pub fn update_agent_list(&mut self, agent_ids: Vec<AgentId>) {
        self.agent_ids = agent_ids;
        
        // Ensure selected agent is in the list
        if !self.agent_ids.contains(&self.selected_agent_id) && !self.agent_ids.is_empty() {
            self.selected_agent_id = self.agent_ids[0];
        }
    }

    /// Draw the UI components
    fn ui(&self, f: &mut Frame) {
        let size = f.size();

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
        self.render_header(f, chunks[0]);

        // Render the content area with conversation history
        self.render_content(f, chunks[1]);

        // Render the input prompt
        self.render_input(f, chunks[2]);
    }

    /// Render the header with agent list
    fn render_header(&self, f: &mut Frame, area: Rect) {
        let agent_spans: Vec<Span> = self
            .agent_ids
            .iter()
            .map(|id| {
                if *id == self.selected_agent_id {
                    Span::styled(
                        format!(" Agent {} ", id),
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        format!(" Agent {} ", id),
                        Style::default().fg(Color::White),
                    )
                }
            })
            .collect();

        let header = Paragraph::new(Line::from(agent_spans))
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
        
        // Create list items from the visible lines
        // Show the most recent `visible_height` lines
        let items: Vec<ListItem> = if total_lines > visible_height {
            // For efficiency: reverse the iterator to start from the end,
            // take the newest visible_height items, then reverse back to display in order
            lines.iter()
                .rev()
                .take(visible_height)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|s| ListItem::new(s.content.as_str()))
                .collect()
        } else {
            // If we have fewer lines than visible height, show all lines
            lines.iter()
                .map(|s| ListItem::new(s.content.as_str()))
                .collect()
        };

        let conversation = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(format!("Conversation ({} lines)", total_lines)))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(conversation, area);
    }

    /// Render the input area
    fn render_input(&self, f: &mut Frame, area: Rect) {
        // Create input text with styling
        let input_style = if self.command_mode {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let input_text = Paragraph::new(self.input.as_str())
            .style(input_style)
            .block(Block::default().borders(Borders::ALL).title("Input"))
            .wrap(Wrap { trim: true });

        f.render_widget(input_text, area);

        // Calculate cursor position
        let cursor_x = self.cursor_position as u16 + 1; // +1 for border
        let cursor_y = area.y + 1; // +1 for border

        // Show cursor at current position
        f.set_cursor(cursor_x, cursor_y);
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
        execute!(stdout, EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let buffer = agent_manager.lock().unwrap().get_agent_buffer(main_agent_id).unwrap();
        let state = TuiState::new(main_agent_id, buffer);

        Ok(Self {
            terminal,
            state,
            agent_manager,
        })
    }

    /// Run the TUI interface
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        while !self.state.should_quit {
            // Draw the UI
            self.terminal.draw(|f| self.state.ui(f))?;

            // Handle events
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key).await?;
                }
            }

            // Process any output from agents
            self.update_agent_output().await?;
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen
        )?;
        self.terminal.show_cursor()?;

        Ok(())
    }

    /// Handle key events
    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match key.code {
            // Exit on Ctrl+c
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
            
            // Submit on Enter
            KeyCode::Enter => {
                let input = std::mem::take(&mut self.state.input);
                self.state.cursor_position = 0;
                
                if !input.is_empty() {
                    if input.starts_with('/') {
                        self.handle_command(&input).await?;
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
            
            // Home
            KeyCode::Home => {
                self.state.cursor_position = 0;
            }
            
            // End
            KeyCode::End => {
                self.state.cursor_position = self.state.input.len();
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
            
            // Ignore other keys
            _ => {}
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

    /// Update the agent output by checking the buffer
    async fn update_agent_output(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // For now, we'll implement a simple polling mechanism
        // In the future, this should use a proper channel or callback system
        
        // Update the agent list
        let agent_ids = {
            let manager = self.agent_manager.lock().unwrap();
            manager.list_agents().iter().map(|v| v.0).collect()
        };
        self.state.update_agent_list(agent_ids);
        
        // In a real implementation, we would have a way to get the agent output
        // For now, this is a placeholder that would be replaced with actual
        // buffer consumption logic
        
        Ok(())
    }
}

impl Drop for TuiInterface {
    fn drop(&mut self) {
        // Ensure terminal is properly cleaned up
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen
        );
    }
}