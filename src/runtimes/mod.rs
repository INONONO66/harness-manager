pub mod auth;
pub mod registry;
pub mod types;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use types::*;

/// Resolve a config directory from a ConfigLocator spec.
fn resolve_config_dir(locator: &ConfigLocator) -> Option<PathBuf> {
    match locator {
        ConfigLocator::EnvOrHome {
            env_var,
            home_relative,
        } => {
            if !env_var.is_empty() {
                if let Ok(dir) = std::env::var(env_var) {
                    let p = PathBuf::from(&dir);
                    if p.is_dir() {
                        return Some(p);
                    }
                }
            }
            dirs::home_dir()
                .map(|h| h.join(home_relative))
                .filter(|p| p.is_dir())
        }
        ConfigLocator::XdgConfig {
            subdir,
            env_override,
        } => {
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
            if let Some(p) = dirs::config_dir()
                .map(|c| c.join(subdir))
                .filter(|p| p.is_dir())
            {
                return Some(p);
            }
            dirs::home_dir()
                .map(|h| h.join(".config").join(subdir))
                .filter(|p| p.is_dir())
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

fn get_version(binary: &PathBuf, version_arg: &str) -> Option<String> {
    let sandbox = version_probe_sandbox();
    fs::create_dir_all(&sandbox).ok()?;
    let home = sandbox.join("home");
    let config = sandbox.join("config");
    let data = sandbox.join("data");
    let cache = sandbox.join("cache");
    let state = sandbox.join("state");
    for dir in [&home, &config, &data, &cache, &state] {
        fs::create_dir_all(dir).ok()?;
    }

    let output = match Command::new(binary)
        .arg(version_arg)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_STATE_HOME", &state)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .env_remove("OPENCODE_CONFIG_DIR")
        .env_remove("PI_CODING_AGENT_DIR")
        .output()
    {
        Ok(output) => output,
        Err(_) => {
            let _ = fs::remove_dir_all(&sandbox);
            return None;
        }
    };
    let _ = fs::remove_dir_all(&sandbox);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout.trim(), stderr.trim());

    combined
        .lines()
        .find(|l| !l.is_empty())
        .map(|l| l.trim().to_string())
}

fn version_probe_sandbox() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("hm-version-{}-{nanos}", std::process::id()))
}

/// Detect a single runtime from its spec.
fn detect_one(spec: &RuntimeSpec) -> DetectedRuntime {
    let binary = find_binary(spec.binary_names);
    let installed = binary.is_some();
    let config_dir = resolve_config_dir(&spec.config_locator);
    let version = binary
        .as_ref()
        .and_then(|b| get_version(b, spec.version_arg));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn get_version_runs_binary_in_sandboxed_home() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "hm-version-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        let script = root.join("version-script");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s\\n' \"$HOME\"\nmkdir -p \"$HOME/touched\"\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        let version = get_version(&script, "--version");

        let sandbox_home = version.expect("script prints sandbox HOME");
        assert!(sandbox_home.contains("hm-version-"));
        assert!(!PathBuf::from(sandbox_home).exists());
        let _ = fs::remove_dir_all(&root);
    }
}
