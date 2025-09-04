//! Main entry point for cargo-status
//!
//! A fast, configurable Rust project status checker that runs multiple cargo tools
//! in parallel and provides unified status reporting with real-time feedback.

use cargo_status::{
    config::{list_profiles, save_profile, Cli, Commands, Config},
    create_all_checks, InteractiveDisplay, Result, StatusEvent,
};
use clap::Parser;
use std::env;
use tokio::task::JoinSet;

/// Show help information when no specific checks are enabled
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
    println!("      --use-profile      Use saved profile");
    println!("      --list-profiles    List available profiles");
    println!("\nFor more help: https://github.com/adamNewell/cargo-status");
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let Commands::Status(args) = cli.command;

    // Safety check: Prevent recursive execution
    if env::var("CARGO_STATUS_RUNNING").is_ok() {
        eprintln!("Error: Detected recursive cargo-status execution!");
        eprintln!("This can happen when running cargo-status on itself.");
        eprintln!("Please run from a different directory.");
        std::process::exit(1);
    }

    // Set environment variable to detect recursion
    // SAFETY: Setting a simple environment variable that we control
    unsafe {
        env::set_var("CARGO_STATUS_RUNNING", "1");
    }

    // Warn if running cargo-status on itself
    if let Ok(manifest_path) = env::current_dir() {
        let cargo_toml = manifest_path.join("Cargo.toml");
        if cargo_toml.exists()
            && let Ok(contents) = std::fs::read_to_string(&cargo_toml)
            && contents.contains("name = \"cargo-status\"") {
            eprintln!(
                "⚠️  Warning: Running cargo-status on the cargo-status project itself!"
            );
            eprintln!("   This may cause excessive process spawning.");
            eprintln!(
                "   Consider running from a different directory or use --help for options.\n"
            );
        }
    }

    // Disable color output if requested
    if args.no_color || env::var("NO_COLOR").is_ok() {
        colored::control::set_override(false);
    }

    // Handle profile operations first
    if args.list_profiles {
        return list_profiles();
    }

    if args.save_profile {
        let profile_name = args.profile.as_deref().unwrap_or("default");
        return save_profile(&args, profile_name);
    }

    // Create unified configuration
    let config = Config::new(args)?;

    // Show help if no checks are enabled
    if !config.has_checks_enabled() {
        show_help();
        return Ok(());
    }

    // Setup display system
    let mut display = InteractiveDisplay::new();
    let event_sender = display.event_sender();

    // Get enabled tool names for display initialization
    let tool_names = config.get_enabled_tools();

    // Initialize display
    display
        .initialize(tool_names.clone())
        .map_err(|e| cargo_status::CargoStatusError::other(format!("Display initialization failed: {}", e)))?;

    // Start the display in a background task
    let display_handle = tokio::spawn(async move {
        if let Err(e) = display.run().await {
            eprintln!("Display error: {}", e);
        }
    });

    // Create all enabled checks
    let all_checks = create_all_checks(&config, event_sender.clone());

    if all_checks.is_empty() {
        eprintln!("No tools available or enabled.");
        return Ok(());
    }

    // Execute checks based on execution mode
    let results = if config.args.sequential || all_checks.len() <= 1 {
        // Run checks sequentially (fmt always first)
        let mut results = Vec::new();
        for check in all_checks {
            let name = check.name.clone();
            let result = check.run().await;
            results.push((name, result));
        }
        results
    } else {
        // Run checks in parallel
        let mut set = JoinSet::new();

        for check in all_checks {
            let name = check.name.clone();
            set.spawn(async move {
                let result = check.run().await;
                (name, result)
            });
        }

        let mut results = Vec::new();
        while let Some(result) = set.join_next().await {
            if let Ok((name, status)) = result {
                results.push((name, status));
            }
        }

        results
    };

    // Send completion event
    let _ = event_sender.send(StatusEvent::AllCompleted);

    // Wait for display to finish
    let _ = display_handle.await;

    // Check if all were successful
    let all_successful = results.iter().all(|(_, status)| {
        matches!(status, cargo_status::CheckStatus::Success { .. })
    });

    // Exit with appropriate code
    if !all_successful {
        std::process::exit(1);
    }

    Ok(())
}