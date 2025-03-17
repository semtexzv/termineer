//! Terminal User Interface (TUI) module for the Termineer application
//!
//! This module implements a Text User Interface using ratatui,
//! providing an interactive and visually appealing interface.

mod commands;
mod events;
mod interface;
mod popup;
mod rendering;
mod state;

// Re-export the main interface
pub use interface::TuiInterface;
