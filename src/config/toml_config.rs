//! TOML configuration structures and loading logic for cargo-status

use serde::Deserialize;
use std::path::Path;

/// Configuration structure for Cargo.toml
#[derive(Deserialize, Default, Debug)]
#[allow(dead_code)]
pub struct CargoTomlConfig {
    #[serde(rename = "cargo-status")]
    pub cargo_status: Option<CargoStatusConfig>,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct CargoStatusConfig {
    #[serde(default)]
    pub checks: ChecksConfig,
    #[serde(default)]
    pub sequential: bool,
    #[serde(default)]
    pub verbose: bool,
    #[serde(default)]
    pub verbose_tools: VerboseTools,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub tool_args: ToolArgs,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct ChecksConfig {
    #[serde(default = "default_true")]
    pub fmt: bool,
    #[serde(default = "default_true")]
    pub check: bool,
    #[serde(default = "default_true")]
    pub clippy: bool,
    #[serde(default = "default_true")]
    pub test: bool,
    #[serde(default = "default_true")]
    pub build: bool,
    #[serde(default)]
    pub doc: bool,
    #[serde(default)]
    pub audit: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct VerboseTools {
    #[serde(default)]
    pub fmt: bool,
    #[serde(default)]
    pub check: bool,
    #[serde(default)]
    pub clippy: bool,
    #[serde(default)]
    pub test: bool,
    #[serde(default)]
    pub build: bool,
    #[serde(default)]
    pub doc: bool,
    #[serde(default)]
    pub audit: bool,
}

#[derive(Deserialize, Default, Debug, Clone)]
pub struct ToolArgs {
    #[serde(default)]
    pub fmt: Vec<String>,
    #[serde(default)]
    pub check: Vec<String>,
    #[serde(default)]
    pub clippy: Vec<String>,
    #[serde(default)]
    pub test: Vec<String>,
    #[serde(default)]
    pub build: Vec<String>,
    #[serde(default)]
    pub doc: Vec<String>,
    #[serde(default)]
    pub audit: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// Load cargo-status configuration from Cargo.toml
pub fn load_cargo_toml_config() -> Option<CargoStatusConfig> {
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

/// Helper function to build command with custom args from TOML config
pub fn build_command_with_config(
    base_cmd: Vec<String>,
    config_args: &[String],
) -> Vec<String> {
    if config_args.is_empty() {
        base_cmd
    } else {
        let mut cmd = base_cmd;
        cmd.extend(config_args.iter().cloned());
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_true() {
        assert!(default_true());
    }

    #[test]
    fn test_build_command_with_config() {
        let base = vec!["cargo".to_string(), "check".to_string()];
        let args = vec!["--lib".to_string()];
        let result = build_command_with_config(base.clone(), &args);
        assert_eq!(result, vec!["cargo", "check", "--lib"]);

        let empty_result = build_command_with_config(base.clone(), &[]);
        assert_eq!(empty_result, base);
    }
}