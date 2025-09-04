use cargo_status::{CargoStatusError, Result, cache::has_tool_cached};
use clap::{Args, Parser, Subcommand};
use colored::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::task::JoinSet;

// Constants for better performance
const DEFAULT_PROFILE_NAME: &str = "default";
const CONFIG_DIR_NAME: &str = "cargo-status";
const PROFILES_FILE_NAME: &str = "profiles.json";
const WARNING_PATTERN: &str = "warning";

/// cargo-status - A fast, configurable Rust project status checker
///
/// This tool provides a unified interface to run common Cargo commands
/// with support for profiles, parallel execution, and granular verbosity control.
#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run cargo development tools with profiles
    Status(StatusArgs),
}

#[derive(Args)]
struct StatusArgs {
    /// Run cargo fmt (use -fv for verbose)
    #[arg(short, long)]
    fmt: bool,

    /// Run cargo check (use -cv for verbose)
    #[arg(short, long)]
    check: bool,

    /// Run cargo clippy (use -lv for verbose)
    #[arg(short = 'l', long)]
    clippy: bool,

    /// Run cargo test (use -tv for verbose)
    #[arg(short, long)]
    test: bool,

    /// Run cargo build (use -bv for verbose)
    #[arg(short, long)]
    build: bool,

    /// Run cargo doc (use -dv for verbose)
    #[arg(short, long)]
    doc: bool,

    /// Run cargo audit (use -uv for verbose)
    #[arg(short = 'u', long)]
    audit: bool,

    /// Run all available checks
    #[arg(short, long)]
    all: bool,

    /// Force sequential execution (fmt always runs first)
    #[arg(long)]
    sequential: bool,

    /// Enable verbose for tools specified with this flag (e.g., -fv = fmt verbose)
    #[arg(short = 'v', long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Save current flags as default profile
    #[arg(long)]
    save_profile: bool,

    /// Load and use saved profile
    #[arg(long)]
    use_profile: bool,

    /// List available profiles
    #[arg(long)]
    list_profiles: bool,

    /// Profile name to save/load
    #[arg(long)]
    profile: Option<String>,

    /// Pass additional arguments to test command (e.g., --test-args="--nocapture")
    #[arg(long)]
    test_args: Option<String>,

    /// Pass additional arguments to build command
    #[arg(long)]
    build_args: Option<String>,

    /// Pass additional arguments to clippy command (before the --)
    #[arg(long)]
    clippy_args: Option<String>,
}

/// Saved profile configuration for cargo-status
///
/// Profiles allow you to save and reuse common flag combinations.
#[derive(Serialize, Deserialize, Default, Clone)]
struct Profile {
    fmt: bool,
    check: bool,
    clippy: bool,
    test: bool,
    build: bool,
    doc: bool,
    audit: bool,
    sequential: bool,
}

// Configuration structure for Cargo.toml
#[derive(Deserialize, Default, Debug)]
#[allow(dead_code)]
struct CargoTomlConfig {
    #[serde(rename = "cargo-status")]
    cargo_status: Option<CargoStatusConfig>,
}

#[derive(Deserialize, Default, Debug, Clone)]
struct CargoStatusConfig {
    #[serde(default)]
    checks: ChecksConfig,
    #[serde(default)]
    sequential: bool,
    #[serde(default)]
    verbose: bool,
    #[serde(default)]
    verbose_tools: VerboseTools,
    #[serde(default)]
    profile: Option<String>,
}

#[derive(Deserialize, Default, Debug, Clone)]
struct ChecksConfig {
    #[serde(default = "default_true")]
    fmt: bool,
    #[serde(default = "default_true")]
    check: bool,
    #[serde(default = "default_true")]
    clippy: bool,
    #[serde(default = "default_true")]
    test: bool,
    #[serde(default = "default_true")]
    build: bool,
    #[serde(default)]
    doc: bool,
    #[serde(default)]
    audit: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
struct VerboseTools {
    #[serde(default)]
    fmt: bool,
    #[serde(default)]
    check: bool,
    #[serde(default)]
    clippy: bool,
    #[serde(default)]
    test: bool,
    #[serde(default)]
    build: bool,
    #[serde(default)]
    doc: bool,
    #[serde(default)]
    audit: bool,
}

fn default_true() -> bool {
    true
}

/// Represents a single cargo command to be executed
///
/// Encapsulates command execution, output parsing, and result reporting.
#[derive(Clone)]
struct StatusCheck {
    name: String,
    command: Vec<String>,
    warning_patterns: Vec<String>,
    verbose: bool,
}

/// Result from running a status check command
///
/// Contains success status, output, and parsed metrics like warnings and errors.
struct CheckResult {
    name: String,
    success: bool,
    #[allow(dead_code)]
    output: String,
    warnings: usize,
    errors: usize,
    test_failed: usize,
    test_passed: usize,
}

impl StatusCheck {
    /// Creates a new StatusCheck with the given name and command
    fn new(name: &str, command: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            command,
            warning_patterns: vec![WARNING_PATTERN.to_string()],
            verbose: false,
        }
    }

    /// Sets custom warning patterns for this check
    fn with_warning_patterns(mut self, patterns: Vec<String>) -> Self {
        self.warning_patterns = patterns;
        self
    }

    /// Enables or disables verbose output for this check
    fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Executes the command and returns parsed results
    ///
    /// Handles both quiet and verbose modes, preserving colors when appropriate.
    async fn run(&self) -> CheckResult {
        let quiet = !self.verbose;
        let mut cmd = Command::new(&self.command[0]);

        // Add color flags for cargo commands to force color output
        let mut args = Vec::new();

        // Force color output for cargo commands
        if self.command[0] == "cargo" && self.command.len() > 1 {
            args.push(self.command[1].clone()); // cargo subcommand (e.g., "check", "clippy")

            // Special handling for nextest
            if self.command[1] == "nextest" {
                // Add rest of nextest arguments first (e.g., "run", "--no-fail-fast")
                for arg in &self.command[2..] {
                    // Skip the --color=always flag we added, we'll handle it below
                    if arg != "--color=always" {
                        args.push(arg.clone());
                    }
                }

                // Add appropriate output control flags based on quiet/verbose mode
                if quiet {
                    // Suppress nextest's output in quiet mode
                    args.push("--status-level".to_string());
                    args.push("none".to_string());
                    args.push("--cargo-quiet".to_string());
                } else {
                    // Keep colors in verbose mode
                    args.push("--color=always".to_string());
                }
            } else if self.command[1] != "fmt" {
                // For other cargo commands (not fmt or nextest)
                args.push("--color=always".to_string());
                // Add rest of the arguments
                for arg in &self.command[2..] {
                    args.push(arg.clone());
                }
            } else {
                // fmt doesn't support color flag
                for arg in &self.command[2..] {
                    args.push(arg.clone());
                }
            }
        } else {
            // Non-cargo commands or edge cases
            args = self.command[1..].to_vec();
        }

        cmd.args(&args);

        if quiet {
            // When quiet, we need to capture output for parsing
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            let output = match cmd.output() {
                Ok(output) => output,
                Err(e) => {
                    return CheckResult {
                        name: self.name.clone(),
                        success: false,
                        output: format!("Failed to run command: {}", e),
                        warnings: 0,
                        errors: 1,
                        test_failed: 0,
                        test_passed: 0,
                    };
                }
            };

            let mut combined_output = String::from_utf8_lossy(&output.stdout).into_owned();
            combined_output.push_str(&String::from_utf8_lossy(&output.stderr));

            self.parse_output(combined_output, output.status.success())
        } else {
            // When verbose, inherit stdio to maintain TTY and colors
            cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

            match cmd.status() {
                Ok(status) => {
                    // For verbose mode, we can't capture output since it's inherited
                    // We need to run the command again quietly to get metrics
                    let mut metric_cmd = Command::new(&self.command[0]);
                    metric_cmd
                        .args(&args)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped());

                    let metric_output =
                        metric_cmd
                            .output()
                            .unwrap_or_else(|_| std::process::Output {
                                status,
                                stdout: Vec::new(),
                                stderr: Vec::new(),
                            });

                    let mut combined_output =
                        String::from_utf8_lossy(&metric_output.stdout).into_owned();
                    combined_output.push_str(&String::from_utf8_lossy(&metric_output.stderr));

                    self.parse_output(combined_output, status.success())
                }
                Err(e) => CheckResult {
                    name: self.name.clone(),
                    success: false,
                    output: format!("Failed to run command: {}", e),
                    warnings: 0,
                    errors: 1,
                    test_failed: 0,
                    test_passed: 0,
                },
            }
        }
    }

    fn parse_output(&self, combined_output: String, success: bool) -> CheckResult {
        let warnings = self
            .warning_patterns
            .iter()
            .map(|pattern| combined_output.matches(pattern).count())
            .sum();

        // Count errors
        let errors =
            combined_output.matches("error:").count() + combined_output.matches("error[").count();

        // Count test results
        let (test_passed, test_failed) = if self.name == "Test" {
            parse_test_results(&combined_output)
        } else {
            (0, 0)
        };

        CheckResult {
            name: self.name.clone(),
            success,
            output: combined_output,
            warnings,
            errors,
            test_failed,
            test_passed,
        }
    }
}

fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or(CargoStatusError::NoConfigDir)?
        .join(CONFIG_DIR_NAME);

    std::fs::create_dir_all(&config_dir).map_err(|e| CargoStatusError::Io {
        context: format!("creating config directory {}", config_dir.display()),
        source: e,
    })?;
    Ok(config_dir.join(PROFILES_FILE_NAME))
}

fn load_cargo_toml_config() -> Option<CargoStatusConfig> {
    // Look for Cargo.toml in current directory
    let cargo_toml_path = Path::new("Cargo.toml");
    if !cargo_toml_path.exists() {
        return None;
    }

    // Read and parse Cargo.toml
    let contents = std::fs::read_to_string(cargo_toml_path).ok()?;
    let cargo_toml: toml::Value = toml::from_str(&contents).ok()?;

    // Package metadata takes precedence over workspace metadata
    // Check package first
    if let Some(config) = cargo_toml
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("cargo-status"))
        .and_then(|c| c.clone().try_into::<CargoStatusConfig>().ok())
    {
        return Some(config);
    }

    // Fall back to workspace metadata if no package metadata
    if let Some(config) = cargo_toml
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("cargo-status"))
        .and_then(|c| c.clone().try_into::<CargoStatusConfig>().ok())
    {
        return Some(config);
    }

    None
}

fn apply_cargo_toml_config(args: &mut StatusArgs, config: &CargoStatusConfig) {
    // Only apply if the flags weren't explicitly set via command line
    // Command line args take precedence over Cargo.toml config

    // For checks, only apply if no specific checks were requested
    if !args.fmt
        && !args.check
        && !args.clippy
        && !args.test
        && !args.build
        && !args.doc
        && !args.audit
        && !args.all
    {
        // Apply default checks from config
        args.fmt = config.checks.fmt;
        args.check = config.checks.check;
        args.clippy = config.checks.clippy;
        args.test = config.checks.test;
        args.build = config.checks.build;
        args.doc = config.checks.doc;
        args.audit = config.checks.audit;
    }

    // Apply sequential if not set
    if !args.sequential {
        args.sequential = config.sequential;
    }

    // Apply verbose if not set
    if args.verbose == 0 && config.verbose {
        args.verbose = 1;
    }

    // Apply profile if not set
    if args.profile.is_none() {
        args.profile = config.profile.clone();
    }
}

fn save_profile(args: &StatusArgs, profile_name: &str) -> Result<()> {
    let config_path = get_config_path()?;

    let mut profiles: HashMap<String, Profile> = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            CargoStatusError::ProfileLoadError {
                path: config_path.clone(),
                source: e,
            }
        })?;
        serde_json::from_str(&content)
            .map_err(|e| CargoStatusError::ProfileParseError { source: e })?
    } else {
        HashMap::new()
    };

    let profile = Profile {
        fmt: args.fmt,
        check: args.check,
        clippy: args.clippy,
        test: args.test,
        build: args.build,
        doc: args.doc,
        audit: args.audit,
        sequential: args.sequential,
    };

    profiles.insert(profile_name.to_string(), profile);

    let json = serde_json::to_string_pretty(&profiles)
        .map_err(|e| CargoStatusError::ProfileParseError { source: e })?;
    std::fs::write(&config_path, json).map_err(|e| CargoStatusError::ProfileSaveError {
        path: config_path,
        source: e,
    })?;

    println!("{} Profile '{}' saved", "✓".green(), profile_name);
    Ok(())
}

fn load_profile(profile_name: &str) -> Result<Profile> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        return Err(CargoStatusError::Other(
            "No profiles found. Save a profile first with --save-profile".to_string(),
        ));
    }

    let content =
        std::fs::read_to_string(&config_path).map_err(|e| CargoStatusError::ProfileLoadError {
            path: config_path.clone(),
            source: e,
        })?;
    let profiles: HashMap<String, Profile> = serde_json::from_str(&content)
        .map_err(|e| CargoStatusError::ProfileParseError { source: e })?;

    profiles
        .get(profile_name)
        .cloned()
        .ok_or_else(|| CargoStatusError::ProfileNotFound {
            name: profile_name.to_string(),
        })
}

fn list_profiles() -> Result<()> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        println!("No profiles found.");
        return Ok(());
    }

    let content =
        std::fs::read_to_string(&config_path).map_err(|e| CargoStatusError::ProfileLoadError {
            path: config_path.clone(),
            source: e,
        })?;
    let profiles: HashMap<String, Profile> = serde_json::from_str(&content)
        .map_err(|e| CargoStatusError::ProfileParseError { source: e })?;

    if profiles.is_empty() {
        println!("No profiles found.");
        return Ok(());
    }

    println!("Available profiles:");
    for (name, profile) in profiles {
        let checks: Vec<String> = [
            (profile.fmt, "fmt"),
            (profile.check, "check"),
            (profile.clippy, "clippy"),
            (profile.test, "test"),
            (profile.build, "build"),
            (profile.doc, "doc"),
            (profile.audit, "audit"),
        ]
        .iter()
        .filter_map(|(enabled, name)| {
            if *enabled {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

        println!("  {} {}: [{}]", "•".blue(), name.bold(), checks.join(", "));
    }

    Ok(())
}

fn show_help() {
    println!("cargo-status: Run cargo development tools with profiles");
    println!("\nUsage: cargo status [OPTIONS]\n");
    println!("Options:");
    println!("  -f, --fmt              Run cargo fmt");
    println!("  -c, --check            Run cargo check");
    println!("  -l, --clippy           Run cargo clippy");
    println!("  -t, --test             Run cargo test (or nextest if available)");
    println!("  -b, --build            Run cargo build");
    println!("  -d, --doc              Run cargo doc");
    println!("  -u, --audit            Run cargo audit (security vulnerabilities)");
    println!("  -a, --all              Run all available checks");
    println!("      --sequential       Force sequential execution");
    println!("  -v, --verbose          Show output for all tools");
    println!("\nVerbose output can be enabled per tool:");
    println!("  -fv                    Run fmt with verbose output");
    println!("  -cv                    Run check with verbose output");
    println!("  -lv                    Run clippy with verbose output");
    println!("  Example: cargo status -fv -t -lv");
    println!("           (fmt verbose, test quiet, clippy verbose)");
    println!("      --save-profile     Save current flags as profile");
    println!("      --use-profile      Load and use saved profile");
    println!("      --list-profiles    List available profiles");
    println!("      --profile <NAME>   Profile name to save/load");
    println!("\nRun 'cargo status --help' for more detailed information");
}

fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip ANSI escape sequence
            if chars.next() == Some('[') {
                // Skip until 'm'
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Parses test output from cargo test or nextest to extract pass/fail counts
///
/// Returns (passed_count, failed_count)
fn parse_test_results(output: &str) -> (usize, usize) {
    // For standard cargo test
    if output.contains("test result:") {
        // Parse "test result: ok. X passed; Y failed; Z ignored"
        for line in output.lines() {
            if line.contains("test result:") {
                let passed = if let Some(idx) = line.find(" passed") {
                    let start = line[..idx].rfind(char::is_numeric);
                    if let Some(start) = start {
                        line[start..idx].trim().parse().unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                };

                let failed = if let Some(idx) = line.find(" failed") {
                    let start = line[..idx].rfind(char::is_numeric);
                    if let Some(start) = start {
                        line[start..idx].trim().parse().unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                };

                return (passed, failed);
            }
        }
    }

    // For nextest
    if output.contains("Summary") {
        // Parse nextest summary line: "Summary [ 0.001s] X tests run: Y passed, Z skipped"
        for line in output.lines() {
            if line.contains("Summary") && line.contains("tests run") {
                // Extract numbers from "X tests run: Y passed"
                // The line might be colored, so strip ANSI codes first
                let clean_line = strip_ansi_escapes(line);

                // Look for the pattern "N passed"
                let passed = if let Some(idx) = clean_line.find("passed") {
                    // Find the number before "passed"
                    let before = &clean_line[..idx];
                    if let Some(colon) = before.rfind(':') {
                        before[colon + 1..].trim().parse().unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                };

                // Count actual FAIL lines since nextest shows "0 failed" when all pass
                let failed = output.matches("FAIL [").count();

                return (passed, failed);
            }
        }
    }

    // Fallback: count individual PASS/FAIL lines
    if output.contains("PASS [") || output.contains("FAIL [") {
        let passed = output.matches("PASS [").count();
        let failed = output.matches("FAIL [").count();
        return (passed, failed);
    }

    (0, 0)
}

fn parse_verbose_tools(cargo_config: Option<&CargoStatusConfig>) -> HashSet<String> {
    let args: Vec<String> = env::args().collect();
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

    // Then override with command-line args
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        // Check for combined short flags like -fv, -cv, etc.
        if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
            if arg.contains('v') {
                // This is a combined flag with verbose
                if arg.contains('f') {
                    verbose_tools.insert("fmt".to_string());
                }
                if arg.contains('c') {
                    verbose_tools.insert("check".to_string());
                }
                if arg.contains('l') {
                    verbose_tools.insert("clippy".to_string());
                }
                if arg.contains('t') {
                    verbose_tools.insert("test".to_string());
                }
                if arg.contains('b') {
                    verbose_tools.insert("build".to_string());
                }
                if arg.contains('d') {
                    verbose_tools.insert("doc".to_string());
                }
                if arg.contains('u') {
                    verbose_tools.insert("audit".to_string());
                }
            }
        }
        // Check for --all -v or --all --verbose
        else if arg == "--all" && i + 1 < args.len() {
            let next = &args[i + 1];
            if next == "-v" || next == "--verbose" {
                // All tools are verbose
                verbose_tools.insert("fmt".to_string());
                verbose_tools.insert("check".to_string());
                verbose_tools.insert("clippy".to_string());
                verbose_tools.insert("test".to_string());
                verbose_tools.insert("build".to_string());
                verbose_tools.insert("doc".to_string());
                verbose_tools.insert("audit".to_string());
            }
        }
        i += 1;
    }

    verbose_tools
}

fn has_nextest() -> bool {
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

fn has_audit() -> bool {
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

fn has_clippy() -> bool {
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

async fn run_fmt_first(verbose: bool) -> CheckResult {
    let fmt_check = StatusCheck::new(
        "Format",
        vec!["cargo".to_string(), "fmt".to_string(), "--all".to_string()],
    )
    .with_verbose(verbose);
    fmt_check.run().await
}

async fn run_parallel_checks(checks: Vec<StatusCheck>) -> Vec<CheckResult> {
    let mut set = JoinSet::new();

    for check in checks {
        let check = Arc::new(check);
        set.spawn(async move { check.run().await });
    }

    let mut results = Vec::new();
    while let Some(result) = set.join_next().await {
        if let Ok(check_result) = result {
            results.push(check_result);
        }
    }

    results
}

async fn run_sequential_checks(checks: Vec<StatusCheck>) -> Vec<CheckResult> {
    let mut results = Vec::new();

    for check in checks {
        let result = check.run().await;
        results.push(result);
    }

    results
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let Commands::Status(mut args) = cli.command;

    // Load configuration from Cargo.toml if present
    let cargo_config = load_cargo_toml_config();
    if let Some(ref config) = cargo_config {
        apply_cargo_toml_config(&mut args, config);
    }

    // Handle profile operations
    if args.list_profiles {
        return list_profiles();
    }

    if args.use_profile {
        let profile_name = args.profile.as_deref().unwrap_or(DEFAULT_PROFILE_NAME);
        let profile = load_profile(profile_name)?;

        // Override args with profile settings
        args.fmt = profile.fmt;
        args.check = profile.check;
        args.clippy = profile.clippy;
        args.test = profile.test;
        args.build = profile.build;
        args.doc = profile.doc;
        args.audit = profile.audit;
        args.sequential = profile.sequential;
    }

    if args.save_profile {
        let profile_name = args.profile.as_deref().unwrap_or(DEFAULT_PROFILE_NAME);
        return save_profile(&args, profile_name);
    }

    // Handle --all flag
    if args.all {
        args.fmt = true;
        args.check = true;
        args.clippy = has_clippy(); // Only include if clippy is available
        args.test = true;
        args.build = true;
        args.doc = true;
        // Include audit if cargo-audit is installed
        if has_audit() {
            args.audit = true;
        }
    }

    // If no flags specified, show help
    if !args.fmt
        && !args.check
        && !args.clippy
        && !args.test
        && !args.build
        && !args.doc
        && !args.audit
    {
        show_help();
        return Ok(());
    }

    // Parse which tools should be verbose
    let verbose_tools = parse_verbose_tools(cargo_config.as_ref());
    let any_verbose = !verbose_tools.is_empty() || args.verbose > 0;

    if any_verbose {
        println!("{}", "Running cargo status checks...".bold());
    }

    // Always run fmt first if requested
    let mut fmt_result = None;
    if args.fmt {
        let verbose_fmt = verbose_tools.contains("fmt") || args.verbose > 0;
        fmt_result = Some(run_fmt_first(verbose_fmt).await);
    }

    // Prepare other checks
    let mut checks = Vec::new();

    if args.check {
        checks.push(
            StatusCheck::new(
                "Check",
                vec![
                    "cargo".to_string(),
                    "check".to_string(),
                    "--workspace".to_string(),
                    "--all-targets".to_string(),
                ],
            )
            .with_verbose(verbose_tools.contains("check") || args.verbose > 0),
        );
    }

    if args.clippy {
        if has_clippy() {
            let mut clippy_cmd = vec![
                "cargo".to_string(),
                "clippy".to_string(),
                "--workspace".to_string(),
                "--all-targets".to_string(),
                "--all-features".to_string(),
            ];

            // Add custom clippy arguments before the -- separator
            if let Some(ref clippy_args) = args.clippy_args {
                for arg in clippy_args.split_whitespace() {
                    clippy_cmd.push(arg.to_string());
                }
            }

            // Add the -- separator and default lint settings
            clippy_cmd.push("--".to_string());
            clippy_cmd.push("-D".to_string());
            clippy_cmd.push("warnings".to_string());

            checks.push(
                StatusCheck::new("Clippy", clippy_cmd)
                    .with_warning_patterns(vec!["warning".to_string(), "help:".to_string()])
                    .with_verbose(verbose_tools.contains("clippy") || args.verbose > 0),
            );
        } else {
            eprintln!("Warning: clippy is not installed. Skipping clippy check.");
            eprintln!("Install it with: rustup component add clippy");
        }
    }

    if args.test {
        let verbose_test = verbose_tools.contains("test") || args.verbose > 0;
        if has_nextest() {
            let mut test_cmd = vec![
                "cargo".to_string(),
                "nextest".to_string(),
                "run".to_string(),
                "--no-fail-fast".to_string(),
                "--color=always".to_string(),
            ];

            // Add custom test arguments if provided
            if let Some(ref test_args) = args.test_args {
                for arg in test_args.split_whitespace() {
                    test_cmd.push(arg.to_string());
                }
            }

            checks.push(StatusCheck::new("Test", test_cmd).with_verbose(verbose_test));
        } else {
            let mut test_cmd = vec![
                "cargo".to_string(),
                "test".to_string(),
                "--workspace".to_string(),
            ];

            // Add custom test arguments if provided
            if let Some(ref test_args) = args.test_args {
                for arg in test_args.split_whitespace() {
                    test_cmd.push(arg.to_string());
                }
            }

            checks.push(StatusCheck::new("Test", test_cmd).with_verbose(verbose_test));
        }
    }

    if args.build {
        let mut build_cmd = vec![
            "cargo".to_string(),
            "build".to_string(),
            "--workspace".to_string(),
            "--all-targets".to_string(),
        ];

        // Add custom build arguments if provided
        if let Some(ref build_args) = args.build_args {
            for arg in build_args.split_whitespace() {
                build_cmd.push(arg.to_string());
            }
        }

        checks.push(
            StatusCheck::new("Build", build_cmd)
                .with_verbose(verbose_tools.contains("build") || args.verbose > 0),
        );
    }

    if args.doc {
        checks.push(
            StatusCheck::new(
                "Doc",
                vec![
                    "cargo".to_string(),
                    "doc".to_string(),
                    "--workspace".to_string(),
                    "--no-deps".to_string(),
                ],
            )
            .with_verbose(verbose_tools.contains("doc") || args.verbose > 0),
        );
    }

    if args.audit {
        if has_audit() {
            checks.push(
                StatusCheck::new("Audit", vec!["cargo".to_string(), "audit".to_string()])
                    .with_verbose(verbose_tools.contains("audit") || args.verbose > 0),
            );
        } else {
            eprintln!("Warning: cargo-audit is not installed. Skipping audit check.");
            eprintln!("Install it with: cargo install cargo-audit");
        }
    }

    // Run checks
    let results = if args.sequential || checks.len() == 1 {
        run_sequential_checks(checks).await
    } else {
        run_parallel_checks(checks).await
    };

    // Print summary
    println!("\n{}", "Status Summary:".bold().underline());

    if let Some(fmt_result) = &fmt_result {
        print_result(fmt_result);
    }

    for result in &results {
        print_result(result);
    }

    // Overall status
    let all_successful =
        fmt_result.as_ref().is_none_or(|r| r.success) && results.iter().all(|r| r.success);

    if all_successful {
        println!("\n{}", "All checks passed! ✓".green().bold());
    } else {
        println!("\n{}", "Some checks failed ✗".red().bold());
        std::process::exit(1);
    }

    Ok(())
}

fn print_result(result: &CheckResult) {
    let status_symbol = if result.success {
        if result.warnings > 0 {
            "⚠".yellow()
        } else {
            "✓".green()
        }
    } else {
        "✗".red()
    };

    // Build metrics string
    let mut metrics = Vec::new();

    // Test-specific metrics
    if result.name == "Test" && (result.test_passed > 0 || result.test_failed > 0) {
        if result.test_failed > 0 {
            metrics.push(
                format!(
                    "{} passed, {} failed",
                    result.test_passed, result.test_failed
                )
                .red()
                .to_string(),
            );
        } else {
            metrics.push(format!("{} passed", result.test_passed).green().to_string());
        }
    }

    // Errors (for build, check, clippy)
    if result.errors > 0 && result.name != "Test" {
        metrics.push(
            format!(
                "{} error{}",
                result.errors,
                if result.errors == 1 { "" } else { "s" }
            )
            .red()
            .to_string(),
        );
    }

    // Warnings
    if result.warnings > 0 {
        metrics.push(
            format!(
                "{} warning{}",
                result.warnings,
                if result.warnings == 1 { "" } else { "s" }
            )
            .yellow()
            .to_string(),
        );
    }

    let metrics_str = if !metrics.is_empty() {
        format!(" ({})", metrics.join(", "))
    } else {
        String::new()
    };

    let message = if result.success {
        if result.warnings > 0 {
            format!("{} {}{}", status_symbol, result.name, metrics_str).yellow()
        } else {
            format!("{} {}{}", status_symbol, result.name, metrics_str).green()
        }
    } else {
        format!("{} {}{}", status_symbol, result.name, metrics_str).red()
    };

    println!("{}", message);
}
