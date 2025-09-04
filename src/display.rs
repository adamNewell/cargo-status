//! Interactive display module for cargo-status
//!
//! Provides real-time terminal UI with progress indicators, spinners,
//! and inline result updates for a professional user experience.

use colored::*;
use crossterm::{
    cursor::{self, MoveTo},
    execute,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Type alias for check state storage
type CheckStateMap = HashMap<String, (u16, CheckStatus, Instant)>;

/// Status of a check execution
#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Pending,
    Running {
        start_time: Instant,
    },
    Success {
        warnings: usize,
        duration: Duration,
    },
    Warning {
        warnings: usize,
        duration: Duration,
    },
    Error {
        errors: usize,
        warnings: usize,
        duration: Duration,
    },
    Failed {
        reason: String,
        duration: Duration,
    },
}

/// Event types for status updates
#[derive(Debug, Clone)]
pub enum StatusEvent {
    CheckStarted { name: String },
    CheckProgress { name: String, message: String },
    CheckCompleted { name: String, status: CheckStatus },
    AllCompleted,
}

/// Manages real-time interactive display for cargo-status
pub struct InteractiveDisplay {
    check_states: Arc<Mutex<CheckStateMap>>,
    is_interactive: bool,
    start_time: Instant,
    event_receiver: mpsc::UnboundedReceiver<StatusEvent>,
    event_sender: mpsc::UnboundedSender<StatusEvent>,
    spinner_frames: Vec<&'static str>,
    base_row: u16,
}

impl InteractiveDisplay {
    /// Creates a new interactive display
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let is_interactive = Self::is_terminal_interactive();

        Self {
            check_states: Arc::new(Mutex::new(HashMap::new())),
            is_interactive,
            start_time: Instant::now(),
            event_receiver,
            event_sender,
            spinner_frames: vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            base_row: 2,
        }
    }

    /// Returns a clone of the event sender for use by check runners
    pub fn event_sender(&self) -> mpsc::UnboundedSender<StatusEvent> {
        self.event_sender.clone()
    }

    /// Detects if we're in an interactive terminal or CI/pipeline
    fn is_terminal_interactive() -> bool {
        // Check for common CI environment variables first (most reliable)
        let ci_vars = [
            "CI",
            "CONTINUOUS_INTEGRATION",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "CIRCLECI",
            "TRAVIS",
            "JENKINS_URL",
            "BUILDKITE",
            "DRONE",
        ];

        for var in &ci_vars {
            if std::env::var(var).is_ok() {
                return false;
            }
        }

        // Check if stdout is a TTY - if yes, definitely interactive
        let is_tty = console::Term::stdout().is_term();
        if is_tty {
            return true;
        }

        // If not a TTY, but we have terminal-like environment variables, assume interactive
        // This helps with IDE integrations and some terminal emulators
        if let Ok(term) = std::env::var("TERM")
            && (term.contains("xterm") || term.contains("color") || term == "screen") {
                return true;
            }

        // Check for other indicators of terminal environment
        if std::env::var("COLORTERM").is_ok() || std::env::var("TERM_PROGRAM").is_ok() {
            return true;
        }

        // Default to false for unknown environments
        false
    }

    /// Initialize the display with tool names
    pub fn initialize(&mut self, tool_names: Vec<String>) -> io::Result<()> {
        if !self.is_interactive {
            self.print_fallback_header(&tool_names);
            return Ok(());
        }

        // Clear screen and hide cursor
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            cursor::Hide,
            MoveTo(0, 0)
        )?;

        println!("{}\n", "Running cargo status checks...".bold().blue());

        // Initialize check positions and states
        let mut states = self.check_states.lock().unwrap();
        for (index, name) in tool_names.iter().enumerate() {
            let row = self.base_row + index as u16;
            states.insert(name.clone(), (row, CheckStatus::Pending, Instant::now()));

            // Draw initial status line
            self.draw_check_line(row, &format!("  {} {} Pending", "◦".dimmed(), name.bold()))?;
        }

        io::stdout().flush()?;
        Ok(())
    }

    /// Draw a line at a specific row
    fn draw_check_line(&self, row: u16, content: &str) -> io::Result<()> {
        execute!(
            io::stdout(),
            MoveTo(0, row),
            Clear(ClearType::CurrentLine),
            Print(content)
        )?;
        io::stdout().flush()?;
        Ok(())
    }

    /// Fallback display for non-interactive terminals
    fn print_fallback_header(&self, tool_names: &[String]) {
        println!("{}", "Running cargo status checks...".bold().blue());
        for name in tool_names {
            println!("  {} {}", "◦".dimmed(), name);
        }
        println!();
    }

    /// Main event processing loop
    pub async fn run(&mut self) -> io::Result<()> {
        if !self.is_interactive {
            return self.run_fallback_mode().await;
        }

        // Start spinner update task
        let states_clone = self.check_states.clone();
        let spinner_frames = self.spinner_frames.clone();
        let spinner_handle = tokio::spawn(async move {
            let mut frame_idx = 0;
            loop {
                tokio::time::sleep(Duration::from_millis(80)).await;

                let states = states_clone.lock().unwrap();
                for (name, (row, status, start_time)) in states.iter() {
                    if let CheckStatus::Running { .. } = status {
                        let spinner = spinner_frames[frame_idx];
                        let elapsed = start_time.elapsed();
                        let line = format!(
                            "  {} {} Running... {}",
                            spinner.blue(),
                            name.bold(),
                            format_duration(elapsed).dimmed()
                        );

                        // Update the line
                        let _ = execute!(
                            io::stdout(),
                            MoveTo(0, *row),
                            Clear(ClearType::CurrentLine),
                            Print(line)
                        );
                    }
                }
                // Flush after updating all spinning lines
                let _ = io::stdout().flush();
                drop(states);

                frame_idx = (frame_idx + 1) % spinner_frames.len();
            }
        });

        // Process events
        while let Some(event) = self.event_receiver.recv().await {
            match event {
                StatusEvent::CheckStarted { name } => {
                    self.handle_check_started(&name)?;
                }
                StatusEvent::CheckProgress { name, message } => {
                    self.handle_check_progress(&name, &message)?;
                }
                StatusEvent::CheckCompleted { name, status } => {
                    self.handle_check_completed(&name, status)?;
                }
                StatusEvent::AllCompleted => {
                    spinner_handle.abort();
                    self.handle_all_completed()?;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Fallback mode for non-interactive terminals
    async fn run_fallback_mode(&mut self) -> io::Result<()> {
        while let Some(event) = self.event_receiver.recv().await {
            match event {
                StatusEvent::CheckStarted { name } => {
                    print!("  {} {} ... ", "◦".blue(), name);
                    io::stdout().flush()?;
                }
                StatusEvent::CheckCompleted { name: _, status } => match status {
                    CheckStatus::Success { duration, warnings } => {
                        if warnings > 0 {
                            println!(
                                "{} ({} warnings, {})",
                                "✓".green(),
                                warnings,
                                format_duration(duration).dimmed()
                            );
                        } else {
                            println!("{} ({})", "✓".green(), format_duration(duration).dimmed());
                        }
                    }
                    CheckStatus::Warning { warnings, duration } => {
                        println!(
                            "{} ({} warnings, {})",
                            "⚠".yellow(),
                            warnings,
                            format_duration(duration).dimmed()
                        );
                    }
                    CheckStatus::Error {
                        errors,
                        warnings,
                        duration,
                    } => {
                        println!(
                            "{} ({} errors, {} warnings, {})",
                            "✗".red(),
                            errors,
                            warnings,
                            format_duration(duration).dimmed()
                        );
                    }
                    CheckStatus::Failed { reason, duration } => {
                        println!(
                            "{} ({}, {})",
                            "✗".red(),
                            reason.red(),
                            format_duration(duration).dimmed()
                        );
                    }
                    _ => {}
                },
                StatusEvent::AllCompleted => {
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle check started event
    fn handle_check_started(&self, name: &str) -> io::Result<()> {
        let mut states = self.check_states.lock().unwrap();
        if let Some((row, _, _)) = states.get(name).cloned() {
            let start_time = Instant::now();
            let status = CheckStatus::Running { start_time };
            states.insert(name.to_string(), (row, status, start_time));
            
            // Draw initial running state
            let line = format!(
                "  {} {} Running...",
                "⠋".blue(),  // First spinner frame
                name.bold()
            );
            
            drop(states); // Release lock before drawing
            self.draw_check_line(row, &line)?;
        }
        Ok(())
    }

    /// Handle check progress event
    fn handle_check_progress(&self, name: &str, message: &str) -> io::Result<()> {
        let states = self.check_states.lock().unwrap();
        if let Some((row, _, start_time)) = states.get(name) {
            let elapsed = start_time.elapsed();
            self.draw_check_line(
                *row,
                &format!(
                    "  {} {} {} {}",
                    "⠼".blue(),
                    name.bold(),
                    message.dimmed(),
                    format_duration(elapsed).dimmed()
                ),
            )?;
        }
        Ok(())
    }

    /// Handle check completed event
    fn handle_check_completed(&self, name: &str, status: CheckStatus) -> io::Result<()> {
        let mut states = self.check_states.lock().unwrap();
        if let Some((row, _, start_time)) = states.get(name).cloned() {
            let line = match &status {
                CheckStatus::Success { duration, warnings } => {
                    if *warnings > 0 {
                        format!(
                            "  {} {} ({} warnings, {})",
                            "✓".green(),
                            name.bold(),
                            warnings,
                            format_duration(*duration).dimmed()
                        )
                    } else {
                        format!(
                            "  {} {} ({})",
                            "✓".green(),
                            name.bold(),
                            format_duration(*duration).dimmed()
                        )
                    }
                }
                CheckStatus::Warning { warnings, duration } => {
                    format!(
                        "  {} {} ({} warnings, {})",
                        "⚠".yellow(),
                        name.bold(),
                        warnings,
                        format_duration(*duration).dimmed()
                    )
                }
                CheckStatus::Error {
                    errors,
                    warnings,
                    duration,
                } => {
                    format!(
                        "  {} {} ({} errors, {} warnings, {})",
                        "✗".red(),
                        name.bold(),
                        errors,
                        warnings,
                        format_duration(*duration).dimmed()
                    )
                }
                CheckStatus::Failed { reason, duration } => {
                    format!(
                        "  {} {} ({}, {})",
                        "✗".red(),
                        name.bold(),
                        reason.red(),
                        format_duration(*duration).dimmed()
                    )
                }
                _ => return Ok(()),
            };

            states.insert(name.to_string(), (row, status, start_time));
            self.draw_check_line(row, &line)?;
        }
        Ok(())
    }

    /// Handle all checks completed
    fn handle_all_completed(&self) -> io::Result<()> {
        if self.is_interactive {
            // Move cursor below all checks
            let states = self.check_states.lock().unwrap();
            let max_row = states.values().map(|(row, _, _)| *row).max().unwrap_or(0);

            execute!(io::stdout(), MoveTo(0, max_row + 2), cursor::Show)?;
        }

        let total_duration = self.start_time.elapsed();
        println!(
            "{} Completed in {}",
            "Summary:".bold().underline(),
            format_duration(total_duration).bold()
        );

        Ok(())
    }

    /// Send status event
    pub fn send_event(
        &self,
        event: StatusEvent,
    ) -> Result<(), mpsc::error::SendError<StatusEvent>> {
        self.event_sender.send(event)
    }

    /// Cleanup on drop or error
    pub fn cleanup(&self) -> io::Result<()> {
        if self.is_interactive {
            execute!(io::stdout(), cursor::Show)?;
        }
        Ok(())
    }
}

impl Default for InteractiveDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for InteractiveDisplay {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Format duration in a human-readable way
fn format_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1000 {
        format!("{}ms", millis)
    } else {
        format!("{:.1}s", duration.as_secs_f32())
    }
}

/// Type aliases for event callbacks
pub type StartCallback = Box<dyn Fn() + Send>;
pub type ProgressCallback = Box<dyn Fn(&str) + Send>;
pub type CompleteCallback = Box<dyn Fn(CheckStatus) + Send>;

/// Integration with StatusCheck
pub fn integrate_with_status_check(
    sender: mpsc::UnboundedSender<StatusEvent>,
    name: String,
) -> (StartCallback, ProgressCallback, CompleteCallback) {
    let name1 = name.clone();
    let name2 = name.clone();
    let name3 = name.clone();
    let sender1 = sender.clone();
    let sender2 = sender.clone();
    let sender3 = sender;

    (
        Box::new(move || {
            let _ = sender1.send(StatusEvent::CheckStarted {
                name: name1.clone(),
            });
        }),
        Box::new(move |msg: &str| {
            let _ = sender2.send(StatusEvent::CheckProgress {
                name: name2.clone(),
                message: msg.to_string(),
            });
        }),
        Box::new(move |status: CheckStatus| {
            let _ = sender3.send(StatusEvent::CheckCompleted {
                name: name3.clone(),
                status,
            });
        }),
    )
}
