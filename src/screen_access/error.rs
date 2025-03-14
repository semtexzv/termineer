//! Error types for screen accessibility operations

use std::fmt;
use std::error::Error as StdError;
use std::result::Result as StdResult;

/// Result type for screen accessibility operations
pub type Result<T> = StdResult<T, Error>;

/// Error type for screen accessibility operations
#[derive(Debug)]
pub enum Error {
    /// The requested window was not found
    WindowNotFound(String),
    
    /// The requested UI element was not found
    ElementNotFound(String),
    
    /// An error occurred in the platform-specific API
    PlatformError(String),
    
    /// An error occurred when capturing a screenshot
    CaptureError(String),
    
    /// An error occurred when processing an image
    ImageError(String),
    
    /// An error with accessing accessibility APIs
    AccessibilityError(String),
    
    /// General error
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WindowNotFound(msg) => write!(f, "Window not found: {}", msg),
            Error::ElementNotFound(msg) => write!(f, "UI element not found: {}", msg),
            Error::PlatformError(msg) => write!(f, "Platform error: {}", msg),
            Error::CaptureError(msg) => write!(f, "Capture error: {}", msg),
            Error::ImageError(msg) => write!(f, "Image error: {}", msg),
            Error::AccessibilityError(msg) => write!(f, "Accessibility error: {}", msg),
            Error::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl StdError for Error {}

/// Convert from a string or string-like type to an Error
impl<T: AsRef<str>> From<T> for Error {
    fn from(msg: T) -> Self {
        Error::Other(msg.as_ref().to_string())
    }
}