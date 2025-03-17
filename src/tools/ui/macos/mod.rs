//! macOS implementation of UI interaction tools
//!
//! This module provides macOS-specific implementation for:
//! - screenshot: Capture screenshots using macOS APIs
//! - screendump: Extract UI structure using macOS Accessibility APIs
//! - input: Send mouse and keyboard inputs using macOS APIs

pub mod input;
pub mod screendump;
pub mod screenshot;
pub mod xml_helpers;
