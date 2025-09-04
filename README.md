# cargo-status

A powerful, configurable cargo subcommand that runs multiple Rust development tools in parallel and provides a unified status report with detailed metrics.

## Features

- 🚀 **Parallel Execution** - Run multiple tools simultaneously for faster feedback
- 📊 **Detailed Metrics** - See test counts, error/warning counts for each tool
- 🔧 **Flexible Configuration** - Configure via CLI, Cargo.toml, or saved profiles
- 🎯 **Selective Verbosity** - Control output verbosity per-tool
- 💾 **Profile Management** - Save and reuse common configurations
- 🎨 **Colored Output** - Clear, color-coded status indicators with proper TTY detection
- 🔍 **Smart Detection** - Automatically detects available tools (nextest, clippy, audit)
- 📦 **Workspace Support** - Works seamlessly with Cargo workspaces
- ⚙️ **Smart Defaults** - Reads project-specific settings from Cargo.toml
- 🔀 **Custom Arguments** - Pass specific arguments to any tool

## Installation

```bash
cargo install cargo-status
```

## Quick Start

```bash
# Run default checks (uses Cargo.toml config, or shows help if no config)
cargo status

# Run all available checks (includes audit if cargo-audit is installed)
cargo status --all

# Run specific checks
cargo status -f -c -t  # fmt, check, test
cargo status --fmt --clippy --build

# Run with verbose output for specific tools
cargo status -fv -t -lv  # fmt verbose, test quiet, clippy verbose

# Pass custom arguments to specific tools
cargo status -t --test-args="--nocapture --test-threads=1"
cargo status -b --build-args="--release"
cargo status -l --clippy-args="--fix"

# Show help
cargo status --help
```

## Output Examples

```
Status Summary:
✓ Format
✓ Check
⚠ Build (5 warnings)
✗ Clippy (2 errors, 14 warnings)
✓ Test (45 passed)
✓ Doc

Some checks failed ✗
```

## ⚠️ Important Usage Notes

### Avoid Running on cargo-status Itself
**Do not run `cargo status` in the cargo-status project directory!** This can cause excessive process spawning as the tool attempts to check itself. Instead:

1. Install cargo-status globally: `cargo install cargo-status`
2. Run it from your other Rust projects
3. If developing cargo-status, test it from a separate test project

The tool includes safety checks to warn about and prevent recursive execution.

## Available Checks

| Flag | Long Form  | Description                                        |
|------|------------|----------------------------------------------------|
| `-f` | `--fmt`    | Run `cargo fmt`                                    |
| `-c` | `--check`  | Run `cargo check`                                  |
| `-l` | `--clippy` | Run `cargo clippy` with strict warnings            |
| `-t` | `--test`   | Run `cargo test` (or `cargo nextest` if available) |
| `-b` | `--build`  | Run `cargo build`                                  |
| `-d` | `--doc`    | Run `cargo doc`                                    |
| `-u` | `--audit`  | Run `cargo audit` for security vulnerabilities     |
| `-a` | `--all`    | Run all available checks (smart detection)         |

## Verbosity Control

### Per-Tool Verbose Mode

Control which tools show output:

```bash
# Combine tool flags with 'v' for verbose
cargo status -fv        # fmt with verbose output
cargo status -fv -cv    # fmt and check with verbose output
cargo status -tv -lv    # test and clippy with verbose output

# Global verbose (all tools)
cargo status --all -v
cargo status --all --verbose
```

### Default Behavior

By default, cargo-status runs in quiet mode, showing only the final status summary. This keeps your terminal clean while still providing essential feedback about pass/fail status and metrics.

## Custom Arguments

You can pass additional arguments to specific tools using dedicated flags:

- `--test-args="..."` - Pass arguments to `cargo test` or `cargo nextest`
- `--build-args="..."` - Pass arguments to `cargo build`
- `--clippy-args="..."` - Pass arguments to `cargo clippy` (before the `--` separator)

### Examples

```bash
# Run tests with specific test binary arguments
cargo status -t --test-args="--nocapture --test-threads=1"

# Build in release mode with specific features
cargo status -b --build-args="--release --features=advanced"

# Run clippy with auto-fix
cargo status -l --clippy-args="--fix"

# Combine multiple custom arguments
cargo status -t -b --test-args="--nocapture" --build-args="--release"

# Test a specific test
cargo status -t --test-args="specific_test_name"
```

**Note**: Arguments are split by whitespace. For complex arguments with spaces, you may need to adjust your quoting.

## Configuration

### Project Configuration (Cargo.toml)

Configure cargo-status per-project by adding settings to your `Cargo.toml`:

```toml
[package.metadata.cargo-status]
# Enable/disable specific checks by default
[package.metadata.cargo-status.checks]
fmt = true
check = true
clippy = true
test = true
build = false  # Don't run build by default
doc = false    # Don't generate docs by default
audit = false  # Don't run audit by default (auto-included with --all if installed)

# Per-tool verbose settings (shows output for specific tools)
[package.metadata.cargo-status.verbose_tools]
fmt = false
check = false
clippy = true   # Always show clippy warnings
test = true     # Always show test output
build = false
doc = false
audit = false

# Other settings
sequential = false  # Run checks in parallel by default
verbose = false     # Global verbose mode
```

### Workspace Configuration

For workspace projects, use `[workspace.metadata.cargo-status]` in your root `Cargo.toml`:

```toml
[workspace.metadata.cargo-status]
[workspace.metadata.cargo-status.checks]
fmt = true
check = true
clippy = true
test = true

[workspace.metadata.cargo-status.verbose_tools]
test = false
clippy = false
```

All workspace members will be checked automatically when running cargo-status.

### Configuration Precedence

1. **Command-line arguments** (highest priority)
2. **Saved profiles** (when using `--use-profile`)
3. **Package metadata** (`[package.metadata.cargo-status]` in package's `Cargo.toml`)
4. **Workspace metadata** (`[workspace.metadata.cargo-status]` in workspace root)
5. **Built-in defaults** (lowest priority)

**Note**: Package configuration completely overrides workspace configuration when both exist. If a workspace sets `verbose = true` for a tool and a package sets `verbose = false`, the package setting wins.

## Profile Management

Save and reuse common configurations:

```bash
# Save current configuration as a profile
cargo status --fmt --check --test --save-profile --profile dev

# Save as default profile
cargo status --fmt --check --save-profile

# Use a saved profile
cargo status --use-profile --profile dev

# Use default profile
cargo status --use-profile

# List all saved profiles
cargo status --list-profiles

# Combine profile with additional flags
cargo status --use-profile --profile dev --build
```

Profiles are stored in `~/.config/cargo-status/profiles.json`

## Advanced Usage

### Sequential Execution

By default, checks run in parallel (except `fmt` which always runs first if included). Force sequential execution for all tools:

```bash
cargo status --all --sequential
```

### CI/CD Integration

```bash
# CI profile with strict checking
cargo status --all --save-profile --profile ci

# In CI pipeline
cargo status --use-profile --profile ci
```

### Custom Workflows

```yaml
# .github/workflows/rust.yml
- name: Run cargo-status checks
  run: |
    cargo install cargo-status
    cargo status --all
```

## Metrics and Status Indicators

- ✓ **Green checkmark** - Check passed without issues
- ⚠ **Yellow warning** - Check passed but with warnings
- ✗ **Red cross** - Check failed with errors
- **(N passed, M failed)** - Test results
- **(N errors, M warnings)** - Build/check/clippy issues

## Requirements

- Rust 1.70.0 or later
- Cargo

### Optional Dependencies

cargo-status automatically detects and uses these tools if available:

- `clippy` - Rust linter (usually pre-installed with rustup)
  - Install with: `rustup component add clippy`
- `cargo-nextest` - Modern test runner with better output
  - Install with: `cargo install cargo-nextest`
- `cargo-audit` - Security vulnerability scanner
  - Install with: `cargo install cargo-audit`

When using `--all`, cargo-status intelligently includes only the tools that are installed.

## Configuration Examples

### Minimal Project (Quick Checks)

```toml
[package.metadata.cargo-status.checks]
fmt = true
check = true
clippy = false  # Skip for quick checks
test = false    # Skip for quick checks
build = false
```

### Library Project

```toml
[package.metadata.cargo-status.checks]
fmt = true
check = true
clippy = true
test = true
build = false  # Libraries don't need build
doc = true     # Documentation is important for libraries

[package.metadata.cargo-status.verbose_tools]
test = true    # Always see test results
doc = true     # See documentation warnings
```

### Application Project

```toml
[package.metadata.cargo-status.checks]
fmt = true
check = true
clippy = true
test = true
build = true   # Ensure it compiles
doc = false    # Internal apps may not need docs

[package.metadata.cargo-status.verbose_tools]
test = true
build = true   # See compilation output

# Custom arguments for tools
[package.metadata.cargo-status.tool_args]
clippy = ["--", "-W", "clippy::pedantic"]  # Enable pedantic lints
test = ["--", "--nocapture"]               # Show test output
build = ["--release"]                      # Build in release mode
```

## Tool Arguments

You can customize arguments passed to each tool using the `tool_args` section in your `Cargo.toml`:

```toml
[package.metadata.cargo-status.tool_args]
fmt = ["--check"]                          # Check formatting without modifying
check = ["--lib"]                          # Only check library code
clippy = ["--", "-W", "clippy::pedantic"]  # Enable additional lints
test = ["--", "--nocapture"]               # Show test output
build = ["--release"]                      # Build optimized
doc = ["--open"]                          # Open docs after generating
audit = ["--ignore", "RUSTSEC-2020-0071"] # Ignore specific advisories
```

### Examples

**Format checking only:**
```toml
[package.metadata.cargo-status.tool_args]
fmt = ["--check"]  # Don't modify files, just check
```

**Strict clippy lints:**
```toml
[package.metadata.cargo-status.tool_args]
clippy = ["--", "-W", "clippy::pedantic", "-W", "clippy::nursery"]
```

**Release builds:**
```toml
[package.metadata.cargo-status.tool_args]
build = ["--release", "--target", "x86_64-unknown-linux-gnu"]
```

## Tips and Tricks

1. **Quick Feedback Loop**: Use `cargo status -fc` for rapid feedback during development
2. **Pre-commit Hook**: Add `cargo status` to your git pre-commit hooks
3. **Custom Aliases**: Add to `~/.cargo/config.toml`:
   ```toml
   [alias]
   s = "status"
   sq = "status -fc"  # quick checks
   sa = "status --all"  # all checks
   ```

## License

MIT OR Apache-2.0

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Repository

https://github.com/adamNewell/cargo-status