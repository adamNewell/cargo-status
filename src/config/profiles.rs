//! Profile management for saving and loading cargo-status configurations

use crate::config::cli::StatusArgs;
use crate::error::{CargoStatusError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// Constants for profile management
const DEFAULT_PROFILE_NAME: &str = "default";
const CONFIG_DIR_NAME: &str = "cargo-status";
const PROFILES_FILE_NAME: &str = "profiles.json";

/// Saved profile configuration for cargo-status
/// 
/// Profiles allow you to save and reuse common flag combinations.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Profile {
    pub fmt: bool,
    pub check: bool,
    pub clippy: bool,
    pub test: bool,
    pub build: bool,
    pub doc: bool,
    pub audit: bool,
    pub sequential: bool,
}

impl Profile {
    /// Create a profile from StatusArgs
    pub fn from_args(args: &StatusArgs) -> Self {
        Self {
            fmt: args.fmt,
            check: args.check,
            clippy: args.clippy,
            test: args.test,
            build: args.build,
            doc: args.doc,
            audit: args.audit,
            sequential: args.sequential,
        }
    }

    /// Apply this profile's settings to StatusArgs
    pub fn apply_to_args(&self, args: &mut StatusArgs) {
        args.fmt = self.fmt;
        args.check = self.check;
        args.clippy = self.clippy;
        args.test = self.test;
        args.build = self.build;
        args.doc = self.doc;
        args.audit = self.audit;
        args.sequential = self.sequential;
    }

    /// Get a list of enabled tools in this profile
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
}

/// Get the path to the profiles configuration file
fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| CargoStatusError::Other("Could not find config directory".to_string()))?
        .join(CONFIG_DIR_NAME);

    std::fs::create_dir_all(&config_dir).map_err(|e| CargoStatusError::ProfileSaveError {
        path: config_dir.clone(),
        source: e,
    })?;

    Ok(config_dir.join(PROFILES_FILE_NAME))
}

/// Load all profiles from the configuration file
fn load_all_profiles() -> Result<HashMap<String, Profile>> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| CargoStatusError::ProfileLoadError {
            path: config_path.clone(),
            source: e,
        })?;

    serde_json::from_str(&content)
        .map_err(|e| CargoStatusError::ProfileParseError { source: e })
}

/// Save all profiles to the configuration file
fn save_all_profiles(profiles: &HashMap<String, Profile>) -> Result<()> {
    let config_path = get_config_path()?;

    let content = serde_json::to_string_pretty(profiles)
        .map_err(|e| CargoStatusError::ProfileParseError { source: e })?;

    std::fs::write(&config_path, content)
        .map_err(|e| CargoStatusError::ProfileSaveError {
            path: config_path,
            source: e,
        })?;

    Ok(())
}

/// Save a profile with the given name
pub fn save_profile(args: &StatusArgs, profile_name: &str) -> Result<()> {
    let mut profiles = load_all_profiles()?;
    let profile = Profile::from_args(args);

    profiles.insert(profile_name.to_string(), profile);
    save_all_profiles(&profiles)?;

    println!("Profile '{}' saved successfully.", profile_name);
    Ok(())
}

/// Load a profile by name
pub fn load_profile(profile_name: &str) -> Result<Profile> {
    let profiles = load_all_profiles()?;

    if profiles.is_empty() {
        return Err(CargoStatusError::Other(
            "No profiles found. Save a profile first with --save-profile".to_string(),
        ));
    }

    profiles
        .get(profile_name)
        .cloned()
        .ok_or_else(|| CargoStatusError::ProfileNotFound {
            name: profile_name.to_string(),
        })
}

/// List all available profiles
pub fn list_profiles() -> Result<()> {
    let profiles = load_all_profiles()?;

    if profiles.is_empty() {
        println!("No profiles found.");
        return Ok(());
    }

    println!("Available profiles:");
    for (name, profile) in &profiles {
        let tools = profile.get_enabled_tools();
        println!("  {}: {}", name, tools.join(", "));
    }

    Ok(())
}

/// Get the default profile name
pub fn get_default_profile_name() -> &'static str {
    DEFAULT_PROFILE_NAME
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_from_args() {
        let args = StatusArgs {
            fmt: true,
            clippy: true,
            ..Default::default()
        };

        let profile = Profile::from_args(&args);
        assert!(profile.fmt);
        assert!(profile.clippy);
        assert!(!profile.check);
    }

    #[test]
    fn test_profile_apply_to_args() {
        let profile = Profile {
            fmt: true,
            test: true,
            ..Default::default()
        };

        let mut args = StatusArgs::default();
        profile.apply_to_args(&mut args);

        assert!(args.fmt);
        assert!(args.test);
        assert!(!args.check);
    }

    #[test]
    fn test_profile_get_enabled_tools() {
        let profile = Profile {
            fmt: true,
            clippy: true,
            ..Default::default()
        };

        let tools = profile.get_enabled_tools();
        assert_eq!(tools, vec!["fmt", "clippy"]);
    }
}