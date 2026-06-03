pub mod types;
pub mod auth;
pub mod registry;

use std::path::PathBuf;
use std::process::Command;

use types::*;

/// Resolve a config directory from a ConfigLocator spec.
fn resolve_config_dir(locator: &ConfigLocator) -> Option<PathBuf> {
    match locator {
        ConfigLocator::EnvOrHome { env_var, home_relative } => {
            if !env_var.is_empty() {
                if let Ok(dir) = std::env::var(env_var) {
                    let p = PathBuf::from(&dir);
                    if p.is_dir() {
                        return Some(p);
                    }
                }
            }
            dirs::home_dir().map(|h| h.join(home_relative)).filter(|p| p.is_dir())
        }
        ConfigLocator::XdgConfig { subdir, env_override } => {
            if !env_override.is_empty() {
                if let Ok(dir) = std::env::var(env_override) {
                    let p = PathBuf::from(&dir);
                    if p.is_dir() {
                        return Some(p);
                    }
                }
            }
            // Try XDG_CONFIG_HOME first, then platform default, then ~/.config fallback.
            // Many CLI tools use ~/.config/ even on macOS where dirs::config_dir()
            // returns ~/Library/Application Support/.
            if let Some(p) = dirs::config_dir().map(|c| c.join(subdir)).filter(|p| p.is_dir()) {
                return Some(p);
            }
            dirs::home_dir().map(|h| h.join(".config").join(subdir)).filter(|p| p.is_dir())
        }
    }
}

/// Find the first matching config file inside the config dir.
fn find_config_file(config_dir: &Option<PathBuf>, candidates: &[&str]) -> Option<PathBuf> {
    let dir = config_dir.as_ref()?;
    for name in candidates {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn find_binary(names: &[&str]) -> Option<PathBuf> {
    for name in names {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
    }
    None
}

/// Get version string from a binary.
fn get_version(binary: &PathBuf, version_arg: &str) -> Option<String> {
    let output = Command::new(binary)
        .arg(version_arg)
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout.trim(), stderr.trim());

    combined
        .lines()
        .find(|l| !l.is_empty())
        .map(|l| l.trim().to_string())
}

/// Detect a single runtime from its spec.
fn detect_one(spec: &RuntimeSpec) -> DetectedRuntime {
    let binary = find_binary(spec.binary_names);
    let installed = binary.is_some();
    let config_dir = resolve_config_dir(&spec.config_locator);
    let version = binary.as_ref().and_then(|b| get_version(b, spec.version_arg));
    let config_path = find_config_file(&config_dir, spec.config_files).or(config_dir.clone());
    let auth_sources = if installed {
        auth::probe_auth_all(spec.auth_probes, config_dir.as_deref())
    } else {
        Vec::new()
    };

    DetectedRuntime {
        name: spec.name.to_string(),
        installed,
        version,
        binary_path: binary,
        config_path: if installed { config_path } else { None },
        auth_sources,
    }
}

/// Run detection for all registered runtimes.
pub fn detect_all() -> Vec<DetectedRuntime> {
    registry::RUNTIMES.iter().map(detect_one).collect()
}
