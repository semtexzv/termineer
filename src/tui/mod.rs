//! Terminal User Interface (TUI) module for the Termineer application
//!
//! This module implements a Text User Interface using ratatui,
//! providing an interactive and visually appealing interface.

mod interface;
mod state;
mod events;
mod rendering;
mod commands;
mod popup;

// Re-export the main interface
pub use interface::TuiInterface;