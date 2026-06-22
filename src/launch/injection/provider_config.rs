use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::config::ResolvedProfile;
use crate::isolation::ensure_safe_write_path;
use crate::runtimes::manifest::ProviderConfigSeedInjection;

use super::json_config::{merge_provider, set_dotted_path};
use super::path::expand_home_template;
use super::validation::{
    effective_endpoint, render_header_value_at_runtime, validate_bearer_value_at_runtime,
    validate_header_name_at_runtime,
};

mod host;
mod secure_write;

#[derive(Debug, Clone)]
pub struct ProviderConfigSeedPreview {
    pub providers: Vec<String>,
    pub endpoint: String,
    pub source: ProviderConfigSeedSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConfigSeedSource {
    Gateway,
    LegacyLlm,
}
/// Pure validation for `provider-config-seed` strategy: checks gateway/legacy
/// fallback, bearer presence, supported_providers membership, and per-provider
/// header value safety. Does NOT touch the filesystem.
pub fn validate_provider_config_seed(
    spec: &ProviderConfigSeedInjection,
    resolved: &ResolvedProfile,
) -> Result<ProviderConfigSeedPreview> {
    let (providers, bearer, base_url, source) = resolve_seed_source(spec, resolved)?;
    validate_bearer_value_at_runtime(&bearer)?;
    let unknown: Vec<&str> = providers
        .iter()
        .filter(|p| !spec.supported_providers.contains(p))
        .map(String::as_str)
        .collect();
    if !unknown.is_empty() {
        anyhow::bail!(
            "gateway lists unsupported providers for this runtime: [{}]. Supported: [{}]",
            unknown.join(", "),
            spec.supported_providers.join(", ")
        );
    }
    ensure_provider_api_key_envs(spec, &providers)?;
    let profile_provider_headers = resolved.gateway.as_ref().map(|gw| &gw.provider_headers);
    for provider in &providers {
        if let Some(manifest_headers) = spec.provider_header_overrides.get(provider) {
            for (name, template) in manifest_headers {
                let _ = render_header_value_at_runtime(name, template, &bearer)?;
            }
        }
        if let Some(headers_map) = profile_provider_headers {
            if let Some(profile_headers) = headers_map.get(provider) {
                for (name, template) in profile_headers {
                    validate_header_name_at_runtime(name)?;
                    let _ = render_header_value_at_runtime(name, template, &bearer)?;
                }
            }
        }
    }
    let effective_strip = resolved
        .gateway
        .as_ref()
        .and_then(|gw| gw.endpoint_strip_v1_override)
        .unwrap_or(spec.endpoint_strip_v1);
    let endpoint = effective_endpoint(&base_url, effective_strip);
    Ok(ProviderConfigSeedPreview {
        providers,
        endpoint,
        source,
    })
}

pub fn apply_provider_config_seed_strategy(
    spec: &ProviderConfigSeedInjection,
    resolved: &ResolvedProfile,
    env: &mut HashMap<String, String>,
    home_dir: &Path,
) -> Result<PathBuf> {
    let (providers, bearer, base_url, _source) = resolve_seed_source(spec, resolved)?;
    validate_bearer_value_at_runtime(&bearer)?;

    let unknown: Vec<&str> = providers
        .iter()
        .filter(|p| !spec.supported_providers.contains(p))
        .map(String::as_str)
        .collect();
    if !unknown.is_empty() {
        anyhow::bail!(
            "gateway lists unsupported providers for this runtime: [{}]. Supported: [{}]",
            unknown.join(", "),
            spec.supported_providers.join(", ")
        );
    }
    ensure_provider_api_key_envs(spec, &providers)?;

    let effective_strip = resolved
        .gateway
        .as_ref()
        .and_then(|gw| gw.endpoint_strip_v1_override)
        .unwrap_or(spec.endpoint_strip_v1);
    let endpoint = effective_endpoint(&base_url, effective_strip);

    let config_path = expand_home_template(&spec.config_path, home_dir);

    ensure_safe_write_path(&config_path, home_dir, "injection.config_path")?;

    let mut body: Value = if config_path.exists() && !spec.overwrite {
        let existing = fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read {}", config_path.display()))?;
        serde_json::from_str(&existing).with_context(|| {
            format!(
                "failed to parse existing provider config {}; refusing to overwrite because overwrite=false",
                config_path.display()
            )
        })?
    } else {
        Value::Object(Map::new())
    };
    if !body.is_object() {
        if !spec.overwrite {
            anyhow::bail!(
                "existing provider config {} is not a JSON object; refusing to overwrite because overwrite=false",
                config_path.display()
            );
        }
        body = Value::Object(Map::new());
    }
    host::merge_host_provider_config(&mut body, &spec.root_key, &config_path, home_dir)?;

    let profile_provider_headers = resolved.gateway.as_ref().map(|gw| &gw.provider_headers);

    for provider in &providers {
        let env_key = spec
            .provider_api_key_envs
            .get(provider)
            .expect("provider env mapping validated before writes");
        env.insert(env_key.clone(), bearer.clone());
        let mut provider_node = Value::Object(Map::new());
        set_dotted_path(
            &mut provider_node,
            &spec.provider_base_url_key,
            Value::String(endpoint.clone()),
        );
        set_dotted_path(
            &mut provider_node,
            &spec.provider_api_key_key,
            Value::String(bearer.clone()),
        );
        if let Some(headers_key) = spec.provider_headers_key.as_deref() {
            let mut headers = Map::new();
            if let Some(manifest_headers) = spec.provider_header_overrides.get(provider) {
                for (name, template) in manifest_headers {
                    let value = render_header_value_at_runtime(name, template, &bearer)?;
                    headers.insert(name.clone(), Value::String(value));
                }
            }
            if let Some(headers_map) = profile_provider_headers {
                if let Some(profile_headers) = headers_map.get(provider) {
                    for (name, template) in profile_headers {
                        validate_header_name_at_runtime(name)?;
                        let value = render_header_value_at_runtime(name, template, &bearer)?;
                        headers.insert(name.clone(), Value::String(value));
                    }
                }
            }
            if !headers.is_empty() {
                set_dotted_path(&mut provider_node, headers_key, Value::Object(headers));
            }
        }
        merge_provider(&mut body, &spec.root_key, provider, provider_node);
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        ensure_safe_write_path(&config_path, home_dir, "injection.config_path")?;
    }
    let pretty = serde_json::to_string_pretty(&body)?;
    secure_write::write_provider_config_file(&config_path, &format!("{pretty}\n"))?;
    Ok(config_path)
}

fn ensure_provider_api_key_envs(
    spec: &ProviderConfigSeedInjection,
    providers: &[String],
) -> Result<()> {
    let missing: Vec<&str> = providers
        .iter()
        .filter(|provider| !spec.provider_api_key_envs.contains_key(*provider))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "runtime manifest is missing provider_api_key_envs for provider(s): [{}]",
            missing.join(", ")
        );
    }
    Ok(())
}

fn resolve_seed_source(
    spec: &ProviderConfigSeedInjection,
    resolved: &ResolvedProfile,
) -> Result<(Vec<String>, String, String, ProviderConfigSeedSource)> {
    if let Some(gateway) = resolved.gateway.as_ref() {
        let bearer = gateway.bearer.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "gateway.bearer is required for provider-config-seed strategy (set [profiles.<name>.gateway].bearer)"
            )
        })?;
        return Ok((
            gateway.providers.clone(),
            bearer.to_string(),
            gateway.base_url.clone(),
            ProviderConfigSeedSource::Gateway,
        ));
    }

    let legacy_provider = spec.legacy_provider.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "provider-config-seed runtime requires a [profiles.<name>.gateway] block (this runtime has no legacy_provider declared, so [profiles.<name>.llm] cannot drive a single-provider seed)"
        )
    })?;
    let endpoint = resolved.endpoint.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "provider-config-seed runtime requires a [profiles.<name>.gateway] block or a [profiles.<name>.llm].endpoint for legacy single-provider seed of '{}'",
            legacy_provider
        )
    })?;
    let bearer = resolved.bearer.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "legacy single-provider seed for '{}' requires [profiles.<name>.llm].bearer",
            legacy_provider
        )
    })?;
    Ok((
        vec![legacy_provider.to_string()],
        bearer.to_string(),
        endpoint.to_string(),
        ProviderConfigSeedSource::LegacyLlm,
    ))
}
