//! Configuration management for cargo-status
//!
//! This module provides unified access to all configuration sources:
//! - Command line arguments (CLI)
//! - TOML configuration files (Cargo.toml)
//! - Saved profiles (profiles.json)

pub mod cli;
pub mod profiles;
pub mod toml_config;

// Re-export commonly used types
pub use cli::{Cli, Commands, StatusArgs};
pub use profiles::{Profile, get_default_profile_name, list_profiles, load_profile, save_profile};
pub use toml_config::{
    build_command_with_config, load_cargo_toml_config, CargoStatusConfig, ChecksConfig,
    ToolArgs, VerboseTools,
};

use crate::error::Result;
use std::collections::HashSet;
use std::env;

/// Unified configuration that combines all sources
pub struct Config {
    pub args: StatusArgs,
    pub toml_config: Option<CargoStatusConfig>,
    pub verbose_tools: HashSet<String>,
}

impl Config {
    /// Create a new configuration by combining all sources
    pub fn new(mut args: StatusArgs) -> Result<Self> {
        // Load TOML configuration
        let toml_config = load_cargo_toml_config();
        
        // Apply TOML configuration if no specific flags were set
        if let Some(ref config) = toml_config {
            apply_toml_config(&mut args, config);
        }

        // Handle profile loading
        if args.use_profile {
            let profile_name = args.profile.clone().unwrap_or_else(|| get_default_profile_name().to_string());
            let profile = load_profile(&profile_name)?;
            profile.apply_to_args(&mut args);
        }

        // Parse verbose tools
        let verbose_tools = parse_verbose_tools(toml_config.as_ref(), &args);

        Ok(Self {
            args,
            toml_config,
            verbose_tools,
        })
    }

    /// Check if any checks are enabled
    pub fn has_checks_enabled(&self) -> bool {
        self.args.has_tool_flags() || self.args.all
    }

    /// Get list of enabled tool names
    pub fn get_enabled_tools(&self) -> Vec<String> {
        if self.args.all {
            // Return all available tools when --all is used with display names
            vec!["Format", "Check", "Clippy", "Test", "Build", "Doc", "Audit"]
                .into_iter()
                .map(String::from)
                .collect()
        } else {
            self.args.get_enabled_tools_display_names()
        }
    }

    /// Check if a tool should run in verbose mode
    pub fn is_tool_verbose(&self, tool: &str) -> bool {
        self.verbose_tools.contains(tool) || self.args.verbose > 0
    }
}

/// Apply TOML configuration to args if no command line flags were set
fn apply_toml_config(args: &mut StatusArgs, config: &CargoStatusConfig) {
    // Only apply if no specific checks were requested
    if !args.has_tool_flags() && !args.all {
        // Apply default checks from config
        args.fmt = config.checks.fmt;
        args.check = config.checks.check;
        args.clippy = config.checks.clippy;
        args.test = config.checks.test;
        args.build = config.checks.build;
        args.doc = config.checks.doc;
        args.audit = config.checks.audit;
    }

    // Apply other settings if not overridden
    if !args.sequential {
        args.sequential = config.sequential;
    }

    // Apply profile setting if not specified
    if args.profile.is_none() {
        args.profile = config.profile.clone();
    }
}

/// Parse which tools should run in verbose mode
fn parse_verbose_tools(cargo_config: Option<&CargoStatusConfig>, _args: &StatusArgs) -> HashSet<String> {
    let cmd_args: Vec<String> = env::args().collect();
    let mut verbose_tools = HashSet::new();

    // First, apply verbose settings from Cargo.toml if present
    if let Some(config) = cargo_config {
        if config.verbose_tools.fmt {
            verbose_tools.insert("fmt".to_string());
        }
        if config.verbose_tools.check {
            verbose_tools.insert("check".to_string());
        }
        if config.verbose_tools.clippy {
            verbose_tools.insert("clippy".to_string());
        }
        if config.verbose_tools.test {
            verbose_tools.insert("test".to_string());
        }
        if config.verbose_tools.build {
            verbose_tools.insert("build".to_string());
        }
        if config.verbose_tools.doc {
            verbose_tools.insert("doc".to_string());
        }
        if config.verbose_tools.audit {
            verbose_tools.insert("audit".to_string());
        }
    }

    // Parse command line for per-tool verbose flags (-fv, -cv, etc.)
    for arg in &cmd_args {
        if arg.starts_with('-') && !arg.starts_with("--") {
            let flags = &arg[1..];
            if flags.contains('v') {
                // Check which tool flag is combined with 'v'
                if flags.contains('f') {
                    verbose_tools.insert("fmt".to_string());
                }
                if flags.contains('c') {
                    verbose_tools.insert("check".to_string());
                }
                if flags.contains('l') {
                    verbose_tools.insert("clippy".to_string());
                }
                if flags.contains('t') {
                    verbose_tools.insert("test".to_string());
                }
                if flags.contains('b') {
                    verbose_tools.insert("build".to_string());
                }
                if flags.contains('d') {
                    verbose_tools.insert("doc".to_string());
                }
                if flags.contains('u') {
                    verbose_tools.insert("audit".to_string());
                }
            }
        }
    }

    verbose_tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let args = StatusArgs::default();
        let config = Config::new(args);
        assert!(config.is_ok());
    }

    #[test]
    fn test_apply_toml_config() {
        let mut args = StatusArgs::default();
        let mut toml_config = CargoStatusConfig::default();
        toml_config.checks.fmt = true;
        toml_config.checks.clippy = true;

        apply_toml_config(&mut args, &toml_config);

        assert!(args.fmt);
        assert!(args.clippy);
        assert!(!args.check); // Should be false by default
    }
}