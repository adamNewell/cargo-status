//! Tool management for cargo-status
//!
//! This module provides a unified interface for running different cargo tools.
//! Each tool is implemented as a separate module with common patterns.

pub mod registry;
pub mod status_check;

// Re-export commonly used types
pub use registry::create_all_checks;
pub use status_check::StatusCheck;
