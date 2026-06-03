use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;

use crate::secrets;

#[derive(Debug, Deserialize, Default)]
pub struct HmConfig {
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Profile {
    pub description: Option<String>,
    pub network: Option<NetworkConfig>,
    pub llm: Option<LlmConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkConfig {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub endpoint: Option<String>,
    pub bearer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    pub name: String,
    pub description: Option<String>,
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub no_proxy: Option<String>,
    pub endpoint: Option<String>,
    pub bearer: Option<String>,
}

fn config_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        let p = config_dir.join("hm").join("config.toml");
        if p.is_file() {
            return p;
        }
    }
    if let Some(home) = dirs::home_dir() {
        let p = home.join(".config").join("hm").join("config.toml");
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("~/.config/hm/config.toml")
}

pub fn load_config() -> anyhow::Result<HmConfig> {
    let path = config_path();
    if !path.is_file() {
        return Ok(HmConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config: {}", path.display()))?;
    let config: HmConfig = toml_edit::de::from_str(&content)
        .with_context(|| format!("failed to parse config: {}", path.display()))?;
    Ok(config)
}

pub fn resolve_profile(config: &HmConfig, name: Option<&str>) -> anyhow::Result<ResolvedProfile> {
    let profile_name = match name {
        Some(n) => n.to_string(),
        None => config.default_profile.clone().ok_or_else(|| {
            anyhow::anyhow!("no profile specified and no default_profile in config")
        })?,
    };

    let profile = config
        .profiles
        .get(&profile_name)
        .ok_or_else(|| anyhow::anyhow!("profile '{}' not found in config", profile_name))?;

    let mut resolved = ResolvedProfile {
        name: profile_name,
        description: profile.description.clone(),
        http_proxy: None,
        https_proxy: None,
        no_proxy: None,
        endpoint: None,
        bearer: None,
    };

    if let Some(ref net) = profile.network {
        resolved.http_proxy = net.http_proxy.clone();
        resolved.https_proxy = net.https_proxy.clone();
        resolved.no_proxy = net.no_proxy.clone();
    }

    if let Some(ref llm) = profile.llm {
        resolved.endpoint = llm.endpoint.clone();
        if let Some(ref bearer_uri) = llm.bearer {
            if bearer_uri.starts_with("secret://") {
                resolved.bearer = Some(secrets::resolve_secret(bearer_uri)?);
            } else {
                resolved.bearer = Some(bearer_uri.clone());
            }
        }
    }

    Ok(resolved)
}

pub fn config_file_path() -> Option<PathBuf> {
    let p = config_path();
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}
