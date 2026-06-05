pub mod auth;
pub mod builtin;
pub mod manifest;
pub mod registry;
pub mod types;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::BTreeSet;

use manifest::{ConfigLocatorRecord, RuntimeRecord};
use registry::RuntimeRegistry;
use types::DetectedRuntime;

fn resolve_config_dir(locator: &ConfigLocatorRecord) -> Option<PathBuf> {
    match locator {
        ConfigLocatorRecord::EnvOrHome { env, home_relative } => {
            if !env.is_empty() {
                if let Ok(dir) = std::env::var(env) {
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
        ConfigLocatorRecord::XdgConfig {
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

fn find_config_file(config_dir: &Option<PathBuf>, candidates: &[String]) -> Option<PathBuf> {
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

fn get_version(
    binary: &PathBuf,
    version_arg: &str,
    env_overrides: &BTreeSet<String>,
) -> Option<String> {
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

    let mut cmd = Command::new(binary);
    cmd.arg(version_arg)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &config)
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_STATE_HOME", &state);
    for key in env_overrides {
        cmd.env_remove(key);
    }

    let output = match cmd.output() {
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

fn collect_runtime_env_overrides(registry: &RuntimeRegistry) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for record in registry.records() {
        match &record.config_locator {
            ConfigLocatorRecord::EnvOrHome { env, .. } if !env.is_empty() => {
                keys.insert(env.clone());
            }
            ConfigLocatorRecord::XdgConfig { env_override, .. } if !env_override.is_empty() => {
                keys.insert(env_override.clone());
            }
            _ => {}
        }
        if let Some(isolation) = record.isolation.as_ref() {
            for (k, _) in &isolation.static_envs {
                keys.insert(k.clone());
            }
        }
        if let Some(isolation) = record.keychain_isolation.as_ref() {
            for (k, _) in &isolation.static_envs {
                keys.insert(k.clone());
            }
        }
    }
    keys
}

fn version_probe_sandbox() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("hm-version-{}-{nanos}", std::process::id()))
}

fn detect_one(record: &RuntimeRecord, env_overrides: &BTreeSet<String>) -> DetectedRuntime {
    let names: Vec<&str> = record.binary_names.iter().map(String::as_str).collect();
    let binary = find_binary(&names);
    let installed = binary.is_some();
    let config_dir = resolve_config_dir(&record.config_locator);
    let version = binary
        .as_ref()
        .and_then(|b| get_version(b, &record.version_arg, env_overrides));
    let config_path = find_config_file(&config_dir, &record.config_files).or(config_dir.clone());
    let auth_sources = if installed {
        auth::probe_auth_all(&record.auth_probes, config_dir.as_deref())
    } else {
        Vec::new()
    };

    DetectedRuntime {
        name: record.name.clone(),
        installed,
        version,
        binary_path: binary,
        config_path: if installed { config_path } else { None },
        auth_sources,
    }
}

pub fn detect_all(registry: &RuntimeRegistry) -> Vec<DetectedRuntime> {
    let env_overrides = collect_runtime_env_overrides(registry);
    registry
        .records()
        .iter()
        .map(|record| detect_one(record, &env_overrides))
        .collect()
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

        let version = get_version(&script, "--version", &BTreeSet::new());

        let sandbox_home = version.expect("script prints sandbox HOME");
        assert!(sandbox_home.contains("hm-version-"));
        assert!(!PathBuf::from(sandbox_home).exists());
        let _ = fs::remove_dir_all(&root);
    }
}
