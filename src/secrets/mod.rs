use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{bail, Context};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn hm_data_dir() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_DATA_HOME") {
        if !v.is_empty() {
            return PathBuf::from(v).join("hm");
        }
    }
    dirs::home_dir()
        .map(|h| h.join(".local").join("share").join("hm"))
        .unwrap_or_else(|| PathBuf::from(".local/share/hm"))
}

fn validate_secret_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name == "."
        || name == ".."
        || name.starts_with('.')
    {
        bail!("invalid secret name: {}", name);
    }
    Ok(())
}

fn secrets_dir() -> PathBuf {
    hm_data_dir().join("secrets")
}

fn secret_path(name: &str) -> anyhow::Result<PathBuf> {
    validate_secret_name(name)?;
    Ok(secrets_dir().join(name))
}

fn ensure_secrets_dir() -> anyhow::Result<PathBuf> {
    let dir = secrets_dir();
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("chmod 700 {}", dir.display()))?;
    Ok(dir)
}

pub fn set_secret(name: &str, value: &str) -> anyhow::Result<()> {
    ensure_secrets_dir()?;
    let path = secret_path(name)?;
    fs::write(&path, value).with_context(|| format!("write secret {}", name))?;
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("chmod 600 {}", path.display()))?;
    Ok(())
}

pub fn get_secret(name: &str) -> anyhow::Result<String> {
    let path = secret_path(name)?;
    fs::read_to_string(&path)
        .with_context(|| format!("secret '{}' not found (run `hm secret set {}`)", name, name))
}

pub fn list_secrets() -> anyhow::Result<Vec<String>> {
    let dir = secrets_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

pub fn remove_secret(name: &str) -> anyhow::Result<bool> {
    let path = secret_path(name)?;
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(&path).with_context(|| format!("remove secret {}", name))?;
    Ok(true)
}

pub fn run_secret_set(name: &str) -> anyhow::Result<()> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read secret from stdin")?;
    let value = input.trim_end_matches(['\r', '\n']).to_string();
    if value.is_empty() {
        bail!("refusing to store empty secret");
    }
    set_secret(name, &value)?;
    eprintln!("Stored secret '{}'.", name);
    Ok(())
}

pub fn run_secret_get(name: &str) -> anyhow::Result<()> {
    print!("{}", get_secret(name)?);
    Ok(())
}

pub fn run_secret_list() -> anyhow::Result<()> {
    for name in list_secrets()? {
        println!("{}", name);
    }
    Ok(())
}

pub fn run_secret_rm(name: &str) -> anyhow::Result<()> {
    if remove_secret(name)? {
        eprintln!("Removed secret '{}'.", name);
    }
    Ok(())
}

pub fn resolve_secret(uri: &str) -> anyhow::Result<String> {
    let stripped = uri
        .strip_prefix("secret://")
        .with_context(|| format!("invalid secret URI (must start with secret://): {}", uri))?;

    if let Some(path) = stripped.strip_prefix("file://") {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read secret file: {}", path))?;
        return Ok(content.trim_end_matches(['\r', '\n']).to_string());
    }

    if let Some(var) = stripped.strip_prefix("env/") {
        return std::env::var(var).with_context(|| format!("secret env var not set: {}", var));
    }

    if let Some(service) = stripped.strip_prefix("keychain/") {
        return resolve_keychain(service);
    }

    if let Some(name) = stripped.strip_prefix("hm/") {
        return get_secret(name).map(|s| s.trim_end_matches(['\r', '\n']).to_string());
    }

    bail!("unknown secret scheme in URI: {}", uri);
}

#[cfg(target_os = "macos")]
fn resolve_keychain(service: &str) -> anyhow::Result<String> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", service, "-w"])
        .output()
        .context("failed to run `security` command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "keychain lookup failed for service '{}': {}",
            service,
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(not(target_os = "macos"))]
fn resolve_keychain(service: &str) -> anyhow::Result<String> {
    bail!(
        "keychain secrets are only supported on macOS (requested service: {})",
        service
    );
}

pub fn mask_secret(value: &str) -> String {
    if value.chars().count() <= 8 {
        return "***".to_string();
    }
    let prefix: String = value.chars().take(4).collect();
    format!("{}***", prefix)
}

#[cfg(test)]
mod mask_tests {
    use super::mask_secret;

    #[test]
    fn mask_secret_returns_stars_for_short_ascii() {
        assert_eq!(mask_secret("abc"), "***");
        assert_eq!(mask_secret("12345678"), "***");
    }

    #[test]
    fn mask_secret_shows_first_four_chars_for_long_ascii() {
        assert_eq!(mask_secret("abcdefghij"), "abcd***");
        assert_eq!(mask_secret("sk-ant-api-12345-abcdef"), "sk-a***");
    }

    #[test]
    fn mask_secret_counts_chars_not_bytes_for_short_non_ascii() {
        assert_eq!(mask_secret("🔐🔑"), "***");
        assert_eq!(mask_secret("한글한글"), "***");
    }

    #[test]
    fn mask_secret_does_not_panic_on_long_non_ascii() {
        let value = "🔐🔑🔐🔑🔐🔑🔐🔑🔐🔑";
        let result = mask_secret(value);
        assert!(result.ends_with("***"));
        assert!(result.starts_with("🔐"));
    }
}
