//! Command line interface definitions for cargo-status

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

/// cargo-status - A fast, configurable Rust project status checker
///
/// This tool provides a unified interface to run common Cargo commands
/// in parallel or sequentially, with configurable output and profiles.
#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Status(StatusArgs),
}

/// Run status checks on your Rust project
#[derive(Parser, Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusArgs {
    /// Run cargo fmt (use -fv for verbose)
    #[arg(short = 'f', long = "fmt")]
    pub fmt: bool,

    /// Run cargo check (use -cv for verbose)
    #[arg(short = 'c', long = "check")]
    pub check: bool,

    /// Run cargo clippy (use -lv for verbose)
    #[arg(short = 'l', long = "clippy")]
    pub clippy: bool,

    /// Run cargo test (use -tv for verbose)
    #[arg(short = 't', long = "test")]
    pub test: bool,

    /// Run cargo build (use -bv for verbose)
    #[arg(short = 'b', long = "build")]
    pub build: bool,

    /// Run cargo doc (use -dv for verbose)
    #[arg(short = 'd', long = "doc")]
    pub doc: bool,

    /// Run cargo audit (use -uv for verbose)
    #[arg(short = 'u', long = "audit")]
    pub audit: bool,

    /// Run all available checks (smart detection of available tools)
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Force sequential execution instead of parallel
    #[arg(long = "sequential")]
    pub sequential: bool,

    /// Global verbose mode (shows output from all tools)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Additional arguments to pass to clippy
    #[arg(long = "clippy-args", value_name = "ARGS")]
    pub clippy_args: Option<String>,

    /// Additional arguments to pass to test command
    #[arg(long = "test-args", value_name = "ARGS")]
    pub test_args: Option<String>,

    /// Additional arguments to pass to build command
    #[arg(long = "build-args", value_name = "ARGS")]
    pub build_args: Option<String>,

    /// Save current configuration as a profile
    #[arg(long = "save-profile")]
    pub save_profile: bool,

    /// Use a saved profile
    #[arg(long = "use-profile")]
    pub use_profile: bool,

    /// Profile name (used with --save-profile or --use-profile)
    #[arg(long = "profile", value_name = "NAME")]
    pub profile: Option<String>,

    /// List all available profiles
    #[arg(long = "list-profiles")]
    pub list_profiles: bool,

    /// Disable colored output
    #[arg(long = "no-color")]
    pub no_color: bool,
}


impl StatusArgs {
    /// Check if any tool-specific flags are set
    pub fn has_tool_flags(&self) -> bool {
        self.fmt || self.check || self.clippy || self.test || self.build || self.doc || self.audit
    }

    /// Check if any profile-related flags are set
    pub fn has_profile_flags(&self) -> bool {
        self.save_profile || self.use_profile || self.list_profiles
    }

    /// Get list of enabled tools (lowercase names)
    pub fn get_enabled_tools(&self) -> Vec<String> {
        let mut tools = Vec::new();
        if self.fmt { tools.push("fmt".to_string()); }
        if self.check { tools.push("check".to_string()); }
        if self.clippy { tools.push("clippy".to_string()); }
        if self.test { tools.push("test".to_string()); }
        if self.build { tools.push("build".to_string()); }
        if self.doc { tools.push("doc".to_string()); }
        if self.audit { tools.push("audit".to_string()); }
        tools
    }

    /// Get list of enabled tools with display names (proper case)
    pub fn get_enabled_tools_display_names(&self) -> Vec<String> {
        let mut tools = Vec::new();
        if self.fmt { tools.push("Format".to_string()); }
        if self.check { tools.push("Check".to_string()); }
        if self.clippy { tools.push("Clippy".to_string()); }
        if self.test { tools.push("Test".to_string()); }
        if self.build { tools.push("Build".to_string()); }
        if self.doc { tools.push("Doc".to_string()); }
        if self.audit { tools.push("Audit".to_string()); }
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_args_default() {
        let args = StatusArgs::default();
        assert!(!args.has_tool_flags());
        assert!(!args.has_profile_flags());
        assert!(args.get_enabled_tools().is_empty());
    }

    #[test]
    fn test_status_args_with_tools() {
        let args = StatusArgs {
            fmt: true,
            clippy: true,
            ..Default::default()
        };
        
        assert!(args.has_tool_flags());
        let tools = args.get_enabled_tools();
        assert_eq!(tools, vec!["fmt", "clippy"]);
    }
}