use anyhow::{bail, Context};

pub fn resolve_secret(uri: &str) -> anyhow::Result<String> {
    let stripped = uri
        .strip_prefix("secret://")
        .with_context(|| format!("invalid secret URI (must start with secret://): {}", uri))?;

    if let Some(path) = stripped.strip_prefix("file://") {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read secret file: {}", path))?;
        return Ok(content.trim_end_matches('\n').to_string());
    }

    if let Some(var) = stripped.strip_prefix("env/") {
        return std::env::var(var).with_context(|| format!("secret env var not set: {}", var));
    }

    if let Some(service) = stripped.strip_prefix("keychain/") {
        return resolve_keychain(service);
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
    if value.len() <= 8 {
        return "***".to_string();
    }
    format!("{}***", &value[..4])
}
