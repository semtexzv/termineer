//! UI interaction tools module
//!
//! This module provides tools for UI interaction:
//! - screenshot: Capture screenshots of windows and screens
//! - screendump: Extract UI structure from applications
//! - input: Send mouse and keyboard inputs to applications

pub mod screenshot;
pub mod screendump;
pub mod input;
pub mod structure;

// Platform-specific implementations
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

// Module exports are re-exported at the top level (tools/mod.rs)