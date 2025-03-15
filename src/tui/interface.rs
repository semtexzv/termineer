//! Main Terminal UI interface implementation

use crate::agent::AgentId;
use crate::tui::{events, rendering, state::TuiState};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use std::io;
use std::time::Duration;

/// TUI interface for the Termineer application
pub struct TuiInterface {
    /// Terminal instance for the TUI
    terminal: Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    /// Application state
    state: TuiState,
}

impl TuiInterface {
    /// Create a new TUI interface
    pub fn new(main_agent_id: AgentId) -> Result<Self, io::Error> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Get the buffer for the main agent
        let buffer = crate::agent::get_agent_buffer(main_agent_id).unwrap();
        
        // Create the TUI state
        let state = TuiState::new(main_agent_id, buffer);

        Ok(Self {
            terminal,
            state,
        })
    }

    /// Run the TUI interface
    pub async fn run(&mut self) -> anyhow::Result<()> {
        while !self.state.should_quit {
            // Process all pending events first
            let mut events_processed = false;

            // Process events in a batch until there are none left
            while event::poll(Duration::from_millis(0))? {
                events_processed = true;
                match event::read()? {
                    Event::Key(key) => {
                        events::handle_key_event(&mut self.state, key).await?;
                    }
                    Event::Mouse(mouse) => {
                        events::handle_mouse_event(&mut self.state, mouse).await?;
                    }
                    Event::Resize(_, _) => {
                        // Terminal resize - bounds will be updated in draw
                    }
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
                rendering::render_ui(&self.state, f);
            })?;

            // If no events were processed, wait a bit to avoid busy-waiting
            if !events_processed {
                event::poll(Duration::from_millis(32))?;
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