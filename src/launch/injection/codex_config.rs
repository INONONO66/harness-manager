use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::config::ResolvedProfile;
use crate::isolation::ensure_safe_write_path;
use crate::runtimes::manifest::CodexConfigSeedInjection;

use super::path::expand_home_template;
use super::provider_config::ProviderConfigSeedSource;
use super::validation::{effective_endpoint, validate_bearer_value_at_runtime};

pub struct CodexConfigSeedPreview {
    pub provider: String,
    pub endpoint: String,
    pub source: ProviderConfigSeedSource,
    pub api_key_env: String,
    pub config_path_display: String,
    pub top_level_writes: Vec<(String, String)>,
}

fn resolve_codex_seed_source(
    spec: &CodexConfigSeedInjection,
    resolved: &ResolvedProfile,
) -> Result<(String, String, ProviderConfigSeedSource)> {
    if let Some(gateway) = resolved.gateway.as_ref() {
        let bearer = gateway.bearer.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "gateway.bearer is required for codex-config-seed strategy (set [profiles.<name>.gateway].bearer)"
            )
        })?;
        if !gateway.providers.contains(&spec.provider) {
            anyhow::bail!(
                "gateway routes providers [{}] but codex requires provider '{}' (supported by runtime: [{}])",
                gateway.providers.join(", "),
                spec.provider,
                spec.supported_providers.join(", ")
            );
        }
        return Ok((
            bearer.to_string(),
            gateway.base_url.clone(),
            ProviderConfigSeedSource::Gateway,
        ));
    }

    let endpoint = resolved.endpoint.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "codex-config-seed requires a [profiles.<name>.gateway] block or [profiles.<name>.llm] endpoint+bearer"
        )
    })?;
    let bearer = resolved.bearer.as_deref().ok_or_else(|| {
        anyhow::anyhow!("codex-config-seed legacy fallback requires [profiles.<name>.llm].bearer")
    })?;
    Ok((
        bearer.to_string(),
        endpoint.to_string(),
        ProviderConfigSeedSource::LegacyLlm,
    ))
}

pub fn validate_codex_config_seed(
    spec: &CodexConfigSeedInjection,
    resolved: &ResolvedProfile,
) -> Result<CodexConfigSeedPreview> {
    let (bearer, base_url, source) = resolve_codex_seed_source(spec, resolved)?;
    validate_bearer_value_at_runtime(&bearer)?;

    let effective_strip = resolved
        .gateway
        .as_ref()
        .and_then(|gw| gw.endpoint_strip_v1_override)
        .unwrap_or(spec.endpoint_strip_v1);
    let endpoint = effective_endpoint(&base_url, effective_strip);

    let top_level_writes = vec![
        (spec.openai_base_url_key.clone(), endpoint.clone()),
        (
            spec.model_provider_key.clone(),
            spec.model_provider_value.clone(),
        ),
    ];

    let config_path_display = spec.config_path.replace("{home}", "<isolation-home>");
    Ok(CodexConfigSeedPreview {
        provider: spec.provider.clone(),
        endpoint,
        source,
        api_key_env: spec.api_key_env.clone(),
        config_path_display,
        top_level_writes,
    })
}

pub fn apply_codex_config_seed_strategy(
    spec: &CodexConfigSeedInjection,
    resolved: &ResolvedProfile,
    env: &mut HashMap<String, String>,
    home_dir: &Path,
) -> Result<PathBuf> {
    let (bearer, base_url, _source) = resolve_codex_seed_source(spec, resolved)?;
    validate_bearer_value_at_runtime(&bearer)?;

    let effective_strip = resolved
        .gateway
        .as_ref()
        .and_then(|gw| gw.endpoint_strip_v1_override)
        .unwrap_or(spec.endpoint_strip_v1);
    let endpoint = effective_endpoint(&base_url, effective_strip);

    let config_path = expand_home_template(&spec.config_path, home_dir);
    ensure_safe_write_path(&config_path, home_dir, "injection.config_path")?;

    let mut doc: toml_edit::DocumentMut = if config_path.exists() && !spec.overwrite {
        let existing = fs::read_to_string(&config_path).with_context(|| {
            format!(
                "failed to read existing codex config {}; refusing to overwrite because overwrite=false",
                config_path.display()
            )
        })?;
        existing.parse().with_context(|| {
            format!(
                "failed to parse existing codex config {}; refusing to overwrite because overwrite=false",
                config_path.display()
            )
        })?
    } else {
        toml_edit::DocumentMut::new()
    };

    doc[spec.openai_base_url_key.as_str()] = toml_edit::value(endpoint.as_str());
    doc[spec.model_provider_key.as_str()] = toml_edit::value(spec.model_provider_value.as_str());

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        ensure_safe_write_path(&config_path, home_dir, "injection.config_path")?;
    }

    let written_path = write_codex_config_file(&config_path, doc.to_string())?;
    seed_codex_api_key_auth(&config_path, home_dir, &bearer)?;
    apply_codex_env(spec, env, bearer);
    Ok(written_path)
}

fn seed_codex_api_key_auth(config_path: &Path, home_dir: &Path, bearer: &str) -> Result<()> {
    let auth_path = config_path.with_file_name("auth.json");
    // symlink_metadata (not exists): runtime launches symlink the host's auth.json,
    // and following that symlink would overwrite the user's real ~/.codex/auth.json.
    if fs::symlink_metadata(&auth_path).is_ok() {
        return Ok(());
    }
    ensure_safe_write_path(&auth_path, home_dir, "injection.auth_path")?;
    let auth = serde_json::json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": bearer,
        "tokens": null,
        "last_refresh": null,
    });
    let serialized = format!("{}\n", serde_json::to_string_pretty(&auth)?);
    write_codex_config_file(&auth_path, serialized)?;
    Ok(())
}

fn write_codex_config_file(config_path: &Path, serialized: String) -> Result<PathBuf> {
    let tmp_path = codex_temp_path(config_path)?;
    let result = (|| -> Result<()> {
        let mut tmp = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp_path)
            .with_context(|| format!("failed to create temp {}", tmp_path.display()))?;
        tmp.write_all(serialized.as_bytes())
            .with_context(|| format!("failed to write temp {}", tmp_path.display()))?;
        tmp.sync_all()
            .with_context(|| format!("failed to sync temp {}", tmp_path.display()))?;
        drop(tmp);
        fs::rename(&tmp_path, config_path).with_context(|| {
            format!(
                "failed to rename {} to {}",
                tmp_path.display(),
                config_path.display()
            )
        })?;
        Ok(())
    })();
    let _ = fs::remove_file(&tmp_path);
    result?;
    Ok(config_path.to_path_buf())
}

fn codex_temp_path(config_path: &Path) -> Result<PathBuf> {
    let parent = config_path
        .parent()
        .with_context(|| format!("config path has no parent: {}", config_path.display()))?;
    let file_name = config_path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| {
            format!(
                "config path has invalid file name: {}",
                config_path.display()
            )
        })?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_nanos();
    Ok(parent.join(format!(
        ".{}.hm-codex-{}-{}.tmp",
        file_name,
        std::process::id(),
        nanos
    )))
}

fn apply_codex_env(
    spec: &CodexConfigSeedInjection,
    env: &mut HashMap<String, String>,
    bearer: String,
) {
    for key in &spec.strip_envs {
        env.remove(key);
    }
    env.insert(spec.api_key_env.clone(), bearer);
}
