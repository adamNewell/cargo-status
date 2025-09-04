//! Error types for cargo-status
//!
//! This module provides comprehensive error handling with detailed error types
//! for all failure modes in the application.

use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

/// Main error type for cargo-status operations
#[derive(Debug, Error)]
pub enum CargoStatusError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Profile '{name}' not found")]
    ProfileNotFound { name: String },

    #[error("Failed to load profile from {path}: {source}")]
    ProfileLoadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to save profile to {path}: {source}")]
    ProfileSaveError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse profile JSON: {source}")]
    ProfileParseError {
        #[source]
        source: serde_json::Error,
    },

    #[error("Command execution failed for '{command}': {reason}")]
    CommandExecution { command: String, reason: String },

    #[error("Tool '{tool}' is not installed. Install with: {install_cmd}")]
    ToolNotInstalled { tool: String, install_cmd: String },

    #[error("Failed to read Cargo.toml: {source}")]
    CargoTomlRead {
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse Cargo.toml: {source}")]
    CargoTomlParse {
        #[source]
        source: toml::de::Error,
    },

    #[error("No configuration directory found")]
    NoConfigDir,

    #[error("IO error: {context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Multiple workspace scanning error: {0}")]
    WorkspaceScan(String),

    #[error("{0}")]
    Other(String),
}

impl CargoStatusError {
    /// Creates a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Creates a command execution error
    pub fn command_execution(command: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::CommandExecution {
            command: command.into(),
            reason: reason.into(),
        }
    }

    /// Creates a tool not installed error with installation instructions
    pub fn tool_not_installed(tool: impl Into<String>, install_cmd: impl Into<String>) -> Self {
        Self::ToolNotInstalled {
            tool: tool.into(),
            install_cmd: install_cmd.into(),
        }
    }

    /// Creates a generic error with a custom message
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, CargoStatusError>;

#[derive(Debug)]
pub struct CommandError {
    pub command: String,
    pub exit_code: Option<i32>,
    pub stderr: String,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Command '{}' failed with exit code {:?}",
            self.command, self.exit_code
        )
    }
}

impl std::error::Error for CommandError {}
