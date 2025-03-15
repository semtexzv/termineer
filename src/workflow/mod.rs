//! Workflow system for orchestrating sequences of operations
//!
//! The workflow system allows users to define and execute sequences of
//! steps such as shell commands, agent messages, file operations, etc.
//! using YAML files stored in the `.termineer/workflows` directory.

pub mod types;
pub mod context;
pub mod executor;
pub mod loader;
pub mod steps;
pub mod cli;

// We don't re-export components to avoid circular dependencies