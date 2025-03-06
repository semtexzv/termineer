//! Serialization utilities for common patterns
//!
//! This module contains utility functions for serialization and deserialization
//! of common patterns used throughout the application.

pub mod element_array;
pub mod string_or_number;

// Re-export common utilities for easier access
pub use element_array::{
    deserialize as deserialize_element_array, serialize as serialize_element_array,
};
// Note: string_or_number functions are not re-exported because they should be used with the module pattern:
// #[serde(with = "string_or_number")]
