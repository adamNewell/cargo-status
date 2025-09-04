//! StatusCheck implementation for executing cargo commands

use crate::display::{CheckStatus, StatusEvent};
use std::process::{Command, Stdio};
use std::time::Instant;
use tokio::sync::mpsc;

// WARNING_PATTERN constant
const WARNING_PATTERN: &str = "warning";

/// Represents a single cargo command to be executed
///
/// Encapsulates command execution, output parsing, and result reporting.
#[derive(Clone)]
pub struct StatusCheck {
    pub name: String,
    pub command: Vec<String>,
    pub warning_patterns: Vec<String>,
    pub verbose: bool,
    pub event_sender: Option<mpsc::UnboundedSender<StatusEvent>>,
}

impl StatusCheck {
    /// Creates a new StatusCheck with the given name and command
    pub fn new(name: &str, command: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            command,
            warning_patterns: vec![WARNING_PATTERN.to_string()],
            verbose: false,
            event_sender: None,
        }
    }

    /// Sets custom warning patterns for this check
    pub fn with_warning_patterns(mut self, patterns: Vec<String>) -> Self {
        self.warning_patterns = patterns;
        self
    }

    /// Enables or disables verbose output for this check
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Sets the event sender for progress updates
    pub fn with_event_sender(mut self, sender: mpsc::UnboundedSender<StatusEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    /// Send a status event if sender is available
    fn send_event(&self, event: StatusEvent) {
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(event);
        }
    }

    /// Executes the command and returns parsed results
    ///
    /// Handles both quiet and verbose modes, preserving colors when appropriate.
    pub async fn run(&self) -> CheckStatus {
        let start_time = Instant::now();

        // Send start event
        self.send_event(StatusEvent::CheckStarted {
            name: self.name.clone(),
        });

        let quiet = !self.verbose;
        let mut cmd = Command::new(&self.command[0]);

        // Build command arguments
        let mut args = Vec::new();

        // Force color output for cargo commands
        if self.command[0] == "cargo" && self.command.len() > 1 {
            args.push(self.command[1].clone()); // cargo subcommand (e.g., "check", "clippy")

            // Special handling for nextest
            if self.command[1] == "nextest" {
                // Add rest of nextest arguments first (e.g., "run", "--no-fail-fast")
                for arg in &self.command[2..] {
                    if arg != "--color=always" {
                        args.push(arg.clone());
                    }
                }

                if quiet {
                    args.push("--status-level".to_string());
                    args.push("none".to_string());
                    args.push("--cargo-quiet".to_string());
                } else {
                    args.push("--color=always".to_string());
                }
            } else if self.command[1] != "fmt" {
                args.push("--color=always".to_string());
                for arg in &self.command[2..] {
                    args.push(arg.clone());
                }
            } else {
                for arg in &self.command[2..] {
                    args.push(arg.clone());
                }
            }
        } else {
            args = self.command[1..].to_vec();
        }

        cmd.args(&args);

        // Send progress event
        self.send_event(StatusEvent::CheckProgress {
            name: self.name.clone(),
            message: "executing...".to_string(),
        });

        // Safety: Check for runaway processes before executing
        if let Ok(output) = std::process::Command::new("sh")
            .args(["-c", "ps aux | grep -c '[c]argo' || echo 0"])
            .output()
            && let Ok(count_str) = String::from_utf8(output.stdout)
            && let Ok(count) = count_str.trim().parse::<usize>()
            && count > 50 {
            let duration = start_time.elapsed();
            let status = CheckStatus::Failed {
                reason: format!(
                    "Too many cargo processes detected ({}), aborting to prevent system overload",
                    count
                ),
                duration,
            };
            self.send_event(StatusEvent::CheckCompleted {
                name: self.name.clone(),
                status: status.clone(),
            });
            return status;
        }

        // Execute command - always capture output for metrics
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let (success, combined_output) = match cmd.output() {
            Ok(output) => {
                // If verbose mode, print the output
                if !quiet {
                    // Print stdout with colors preserved
                    if !output.stdout.is_empty() {
                        print!("{}", String::from_utf8_lossy(&output.stdout));
                    }
                    // Print stderr with colors preserved
                    if !output.stderr.is_empty() {
                        eprint!("{}", String::from_utf8_lossy(&output.stderr));
                    }
                }

                let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
                combined.push_str(&String::from_utf8_lossy(&output.stderr));
                (output.status.success(), combined)
            }
            Err(e) => {
                let duration = start_time.elapsed();
                let status = CheckStatus::Failed {
                    reason: format!("Failed to run command: {}", e),
                    duration,
                };

                self.send_event(StatusEvent::CheckCompleted {
                    name: self.name.clone(),
                    status: status.clone(),
                });

                return status;
            }
        };

        // Parse results
        let duration = start_time.elapsed();
        let warnings = self
            .warning_patterns
            .iter()
            .map(|pattern| combined_output.matches(pattern).count())
            .sum();

        let errors =
            combined_output.matches("error:").count() + combined_output.matches("error[").count();

        let (_test_passed, test_failed) = if self.name == "Test" {
            parse_test_results(&combined_output)
        } else {
            (0, 0)
        };

        // Determine final status
        let status = if !success || errors > 0 || test_failed > 0 {
            CheckStatus::Error {
                errors: errors + test_failed,
                warnings,
                duration,
            }
        } else if warnings > 0 {
            CheckStatus::Warning { warnings, duration }
        } else {
            CheckStatus::Success {
                warnings: 0,
                duration,
            }
        };

        // Send completion event
        self.send_event(StatusEvent::CheckCompleted {
            name: self.name.clone(),
            status: status.clone(),
        });

        status
    }
}

/// Parse test results from output to extract passed/failed counts
fn parse_test_results(output: &str) -> (usize, usize) {
    // Look for patterns like "test result: ok. X passed; Y failed"
    if let Some(result_line) = output.lines().find(|line| line.contains("test result:")) {
        let words: Vec<&str> = result_line.split_whitespace().collect();
        
        let mut passed = 0;
        let mut failed = 0;
        
        for i in 0..words.len() {
            if words[i] == "passed;" && i > 0
                && let Ok(count) = words[i - 1].parse::<usize>() {
                passed = count;
            } else if words[i] == "failed;" && i > 0
                && let Ok(count) = words[i - 1].parse::<usize>() {
                failed = count;
            }
        }

        return (passed, failed);
    }

    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_test_results() {
        let output = "test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 12);
        assert_eq!(failed, 0);

        let output2 = "test result: FAILED. 8 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out";
        let (passed2, failed2) = parse_test_results(output2);
        assert_eq!(passed2, 8);
        assert_eq!(failed2, 3);
    }

    #[test]
    fn test_status_check_creation() {
        let cmd = vec!["cargo".to_string(), "check".to_string()];
        let check = StatusCheck::new("Test Check", cmd);
        
        assert_eq!(check.name, "Test Check");
        assert!(!check.verbose);
        assert_eq!(check.warning_patterns, vec!["warning"]);
    }
}