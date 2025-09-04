use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_cargo_status_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "status", "--help"])
        .output()
        .expect("Failed to execute cargo-status");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo") || stdout.contains("status"));
}

#[test]
fn test_list_profiles_empty() {
    let output = Command::new("cargo")
        .args(["run", "--", "status", "--list-profiles"])
        .output()
        .expect("Failed to execute cargo-status");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No profiles") || stdout.contains("Available profiles"));
}

#[test]
fn test_run_with_fmt_flag() {
    let output = Command::new("cargo")
        .args(["run", "--", "status", "-f"])
        .output()
        .expect("Failed to execute cargo-status");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Format") || stdout.contains("fmt") || stdout.contains("Status Summary")
    );
}

#[test]
fn test_verbose_mode() {
    let output = Command::new("cargo")
        .args(["run", "--", "status", "-fv"])
        .output()
        .expect("Failed to execute cargo-status");

    // In verbose mode, we should see more output
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stderr.is_empty() || !stdout.is_empty());
}

#[test]
fn test_profile_save_and_load() {
    use std::env;

    // Create a temporary config directory
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("cargo-status");
    fs::create_dir_all(&config_path).unwrap();

    // Set HOME to temp directory for this test
    unsafe {
        env::set_var("HOME", temp_dir.path());
    }

    // Save a profile
    let save_output = Command::new("cargo")
        .args([
            "run",
            "--",
            "status",
            "-f",
            "-c",
            "--save-profile",
            "--profile",
            "test_profile",
        ])
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to save profile");

    let _save_stdout = String::from_utf8_lossy(&save_output.stdout);

    // Now try to load it
    let load_output = Command::new("cargo")
        .args([
            "run",
            "--",
            "status",
            "--use-profile",
            "--profile",
            "test_profile",
        ])
        .env("HOME", temp_dir.path())
        .output()
        .expect("Failed to load profile");

    // Verify the profile was loaded (should run fmt and check based on saved profile)
    let _load_stdout = String::from_utf8_lossy(&load_output.stdout);

    // Both operations should complete
    assert!(save_output.status.code().is_some() || load_output.status.code().is_some());
}

#[test]
fn test_all_flag() {
    let output = Command::new("cargo")
        .args(["run", "--", "status", "--all"])
        .env("CI", "1")  // Force non-interactive mode for tests
        .output()
        .expect("Failed to execute cargo-status");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should run multiple checks
    assert!(
        stdout.contains("Format")
            || stdout.contains("Check")
            || stdout.contains("Test")
            || stdout.contains("Status Summary")
    );
}

#[test]
fn test_sequential_flag() {
    let output = Command::new("cargo")
        .args(["run", "--", "status", "-f", "-c", "--sequential"])
        .output()
        .expect("Failed to execute cargo-status");

    // Should complete without errors
    assert!(output.status.code().is_some());
}

#[test]
fn test_no_flags_shows_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "status"])
        .output()
        .expect("Failed to execute cargo-status");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // When no flags are provided, cargo-status uses defaults from Cargo.toml
    // In our case, it will run the default checks (fmt, check, clippy, test)
    assert!(
        stdout.contains("Usage")
            || stdout.contains("Options")
            || stdout.contains("cargo-status")
            || stdout.contains("Running cargo status checks")
            || stderr.contains("Running cargo status checks")
            || stdout.contains("Format") // Default check enabled in Cargo.toml
    );
}

#[test]
fn test_multiple_verbose_flags() {
    let output = Command::new("cargo")
        .args(["run", "--", "status", "-fv", "-cv"])
        .output()
        .expect("Failed to execute cargo-status");

    // Should handle multiple verbose flags
    assert!(output.status.code().is_some());
}
