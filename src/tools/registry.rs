//! Tool registry for managing available cargo tools

use crate::cache::has_tool_cached;
use crate::config::{build_command_with_config, Config};
use crate::display::StatusEvent;
use crate::tools::status_check::StatusCheck;
use std::process::{Command, Stdio};
use tokio::sync::mpsc;

/// Registry for managing available tools and creating checks
pub struct ToolRegistry;

impl ToolRegistry {
    /// Check if clippy is available
    pub fn has_clippy() -> bool {
        has_tool_cached("clippy", || {
            Command::new("cargo")
                .args(["clippy", "--version"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        })
    }

    /// Check if cargo-audit is available
    pub fn has_audit() -> bool {
        has_tool_cached("audit", || {
            Command::new("cargo")
                .args(["audit", "--version"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        })
    }

    /// Check if cargo-nextest is available
    pub fn has_nextest() -> bool {
        has_tool_cached("nextest", || {
            Command::new("cargo")
                .args(["nextest", "--version"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        })
    }

    /// Create a StatusCheck for the format tool
    pub fn create_fmt_check(
        config: &Config,
        event_sender: mpsc::UnboundedSender<StatusEvent>,
    ) -> StatusCheck {
        let base_cmd = vec!["cargo".to_string(), "fmt".to_string(), "--all".to_string()];
        let fmt_cmd = if let Some(ref toml_config) = config.toml_config {
            build_command_with_config(base_cmd, &toml_config.tool_args.fmt)
        } else {
            base_cmd
        };

        StatusCheck::new("Format", fmt_cmd)
            .with_verbose(config.is_tool_verbose("fmt"))
            .with_event_sender(event_sender)
    }

    /// Create a StatusCheck for the check tool
    pub fn create_check_check(
        config: &Config,
        event_sender: mpsc::UnboundedSender<StatusEvent>,
    ) -> StatusCheck {
        let base_cmd = vec![
            "cargo".to_string(),
            "check".to_string(),
            "--workspace".to_string(),
            "--all-targets".to_string(),
        ];
        let check_cmd = if let Some(ref toml_config) = config.toml_config {
            build_command_with_config(base_cmd, &toml_config.tool_args.check)
        } else {
            base_cmd
        };

        StatusCheck::new("Check", check_cmd)
            .with_verbose(config.is_tool_verbose("check"))
            .with_event_sender(event_sender)
    }

    /// Create a StatusCheck for the clippy tool
    pub fn create_clippy_check(
        config: &Config,
        event_sender: mpsc::UnboundedSender<StatusEvent>,
    ) -> Option<StatusCheck> {
        if !Self::has_clippy() {
            eprintln!("Warning: clippy is not installed. Skipping clippy check.");
            eprintln!("Install it with: rustup component add clippy");
            return None;
        }

        let mut base_cmd = vec![
            "cargo".to_string(),
            "clippy".to_string(),
            "--workspace".to_string(),
            "--all-targets".to_string(),
            "--all-features".to_string(),
        ];

        // Add custom clippy arguments from command line before the -- separator
        if let Some(ref clippy_args) = config.args.clippy_args {
            for arg in clippy_args.split_whitespace() {
                base_cmd.push(arg.to_string());
            }
        }

        // Add custom clippy arguments from TOML config
        if let Some(ref toml_config) = config.toml_config {
            base_cmd.extend(toml_config.tool_args.clippy.iter().cloned());
        }

        // Add the -- separator and default lint settings
        base_cmd.push("--".to_string());
        base_cmd.push("-D".to_string());
        base_cmd.push("warnings".to_string());

        Some(
            StatusCheck::new("Clippy", base_cmd)
                .with_warning_patterns(vec!["warning".to_string(), "help:".to_string()])
                .with_verbose(config.is_tool_verbose("clippy"))
                .with_event_sender(event_sender),
        )
    }

    /// Create a StatusCheck for the test tool
    pub fn create_test_check(
        config: &Config,
        event_sender: mpsc::UnboundedSender<StatusEvent>,
    ) -> StatusCheck {
        let mut test_cmd = if Self::has_nextest() {
            vec![
                "cargo".to_string(),
                "nextest".to_string(),
                "run".to_string(),
                "--no-fail-fast".to_string(),
                "--color=always".to_string(),
            ]
        } else {
            vec![
                "cargo".to_string(),
                "test".to_string(),
                "--workspace".to_string(),
            ]
        };

        // Add custom test arguments from command line
        if let Some(ref test_args) = config.args.test_args {
            for arg in test_args.split_whitespace() {
                test_cmd.push(arg.to_string());
            }
        }

        // Add custom test arguments from TOML config
        if let Some(ref toml_config) = config.toml_config {
            test_cmd.extend(toml_config.tool_args.test.iter().cloned());
        }

        StatusCheck::new("Test", test_cmd)
            .with_verbose(config.is_tool_verbose("test"))
            .with_event_sender(event_sender)
    }

    /// Create a StatusCheck for the build tool
    pub fn create_build_check(
        config: &Config,
        event_sender: mpsc::UnboundedSender<StatusEvent>,
    ) -> StatusCheck {
        let mut build_cmd = vec![
            "cargo".to_string(),
            "build".to_string(),
            "--workspace".to_string(),
            "--all-targets".to_string(),
        ];

        // Add custom build arguments from command line
        if let Some(ref build_args) = config.args.build_args {
            for arg in build_args.split_whitespace() {
                build_cmd.push(arg.to_string());
            }
        }

        // Add custom build arguments from TOML config
        if let Some(ref toml_config) = config.toml_config {
            build_cmd.extend(toml_config.tool_args.build.iter().cloned());
        }

        StatusCheck::new("Build", build_cmd)
            .with_verbose(config.is_tool_verbose("build"))
            .with_event_sender(event_sender)
    }

    /// Create a StatusCheck for the doc tool
    pub fn create_doc_check(
        config: &Config,
        event_sender: mpsc::UnboundedSender<StatusEvent>,
    ) -> StatusCheck {
        let base_cmd = vec![
            "cargo".to_string(),
            "doc".to_string(),
            "--workspace".to_string(),
            "--no-deps".to_string(),
        ];
        let doc_cmd = if let Some(ref toml_config) = config.toml_config {
            build_command_with_config(base_cmd, &toml_config.tool_args.doc)
        } else {
            base_cmd
        };

        StatusCheck::new("Doc", doc_cmd)
            .with_verbose(config.is_tool_verbose("doc"))
            .with_event_sender(event_sender)
    }

    /// Create a StatusCheck for the audit tool
    pub fn create_audit_check(
        config: &Config,
        event_sender: mpsc::UnboundedSender<StatusEvent>,
    ) -> Option<StatusCheck> {
        if !Self::has_audit() {
            eprintln!("Warning: cargo-audit is not installed. Skipping audit check.");
            eprintln!("Install it with: cargo install cargo-audit");
            return None;
        }

        let base_cmd = vec!["cargo".to_string(), "audit".to_string()];
        let audit_cmd = if let Some(ref toml_config) = config.toml_config {
            build_command_with_config(base_cmd, &toml_config.tool_args.audit)
        } else {
            base_cmd
        };

        Some(
            StatusCheck::new("Audit", audit_cmd)
                .with_verbose(config.is_tool_verbose("audit"))
                .with_event_sender(event_sender),
        )
    }
}

/// Create all enabled checks based on configuration
pub fn create_all_checks(
    config: &Config,
    event_sender: mpsc::UnboundedSender<StatusEvent>,
) -> Vec<StatusCheck> {
    let mut checks = Vec::new();

    // Always run fmt first if requested
    if config.args.fmt || config.args.all {
        checks.push(ToolRegistry::create_fmt_check(config, event_sender.clone()));
    }

    // Add other checks
    if config.args.check || config.args.all {
        checks.push(ToolRegistry::create_check_check(config, event_sender.clone()));
    }

    if (config.args.clippy || config.args.all)
        && let Some(clippy_check) = ToolRegistry::create_clippy_check(config, event_sender.clone()) {
        checks.push(clippy_check);
    }

    if config.args.test || config.args.all {
        checks.push(ToolRegistry::create_test_check(config, event_sender.clone()));
    }

    if config.args.build || config.args.all {
        checks.push(ToolRegistry::create_build_check(config, event_sender.clone()));
    }

    if config.args.doc || config.args.all {
        checks.push(ToolRegistry::create_doc_check(config, event_sender.clone()));
    }

    if (config.args.audit || config.args.all)
        && let Some(audit_check) = ToolRegistry::create_audit_check(config, event_sender.clone()) {
        checks.push(audit_check);
    }

    checks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, StatusArgs};
    use std::collections::HashSet;

    #[test]
    fn test_create_checks_empty() {
        // Create config without TOML defaults by passing explicit false values
        let config = Config {
            args: StatusArgs::default(),
            toml_config: None,
            verbose_tools: HashSet::new(),
        };
        let (sender, _receiver) = mpsc::unbounded_channel();
        let checks = create_all_checks(&config, sender);
        assert!(checks.is_empty());
    }

    #[test]
    fn test_create_fmt_check() {
        let args = StatusArgs {
            fmt: true,
            ..Default::default()
        };
        let config = Config::new(args).unwrap();
        let (sender, _receiver) = mpsc::unbounded_channel();
        let checks = create_all_checks(&config, sender);
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].name, "Format");
    }
}