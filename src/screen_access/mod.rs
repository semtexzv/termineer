//! Screen accessibility module for window management and UI inspection
//!
//! This module provides a cross-platform interface for:
//! - Window enumeration and targeting
//! - UI element hierarchy inspection
//! - Screen and window capture

mod error;
mod window;
mod element;
mod manager;
mod image;

// Re-export public types
pub use error::{Error, Result};
pub use window::Window;
pub use element::UIElement;
pub use manager::WindowManager;
pub use image::Image;

// Platform-specific implementations
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

/// Create a window manager for the current platform
pub fn create_window_manager() -> Box<dyn WindowManager> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSWindowManager::new())
    }
    
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsWindowManager::new())
    }
    
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxWindowManager::new())
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        compile_error!("Unsupported platform");
    }
}