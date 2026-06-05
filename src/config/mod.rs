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
    pub gateway: Option<GatewayConfig>,
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

#[derive(Debug, Deserialize, Clone)]
pub struct GatewayConfig {
    pub base_url: String,
    pub bearer: Option<String>,
    pub providers: Vec<String>,
    #[serde(default)]
    pub endpoint_strip_v1: Option<bool>,
    #[serde(default)]
    pub provider_headers: HashMap<String, HashMap<String, String>>,
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
    pub gateway: Option<ResolvedGateway>,
}

#[derive(Debug, Clone)]
pub struct ResolvedGateway {
    pub base_url: String,
    pub bearer: Option<String>,
    pub providers: Vec<String>,
    pub endpoint_strip_v1_override: Option<bool>,
    pub provider_headers: HashMap<String, HashMap<String, String>>,
}

fn config_path_from(xdg_config_home: Option<&PathBuf>, home: Option<&PathBuf>) -> PathBuf {
    if let Some(config_dir) = xdg_config_home {
        let p = config_dir.join("hm").join("config.toml");
        return p;
    }
    if let Some(config_dir) = dirs::config_dir() {
        let p = config_dir.join("hm").join("config.toml");
        if p.is_file() {
            return p;
        }
    }
    if let Some(home) = home {
        let p = home.join(".config").join("hm").join("config.toml");
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("~/.config/hm/config.toml")
}

fn config_path() -> PathBuf {
    let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let home = dirs::home_dir();
    config_path_from(xdg_config_home.as_ref(), home.as_ref())
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
        name: profile_name.clone(),
        description: profile.description.clone(),
        http_proxy: None,
        https_proxy: None,
        no_proxy: None,
        endpoint: None,
        bearer: None,
        gateway: None,
    };

    if let Some(ref net) = profile.network {
        resolved.http_proxy = net.http_proxy.clone();
        resolved.https_proxy = net.https_proxy.clone();
        resolved.no_proxy = net.no_proxy.clone();
    }

    if let Some(ref llm) = profile.llm {
        resolved.endpoint = llm.endpoint.clone();
        if let Some(ref bearer_uri) = llm.bearer {
            resolved.bearer = Some(resolve_secret_value(bearer_uri)?);
        }
    }

    if let Some(ref gw) = profile.gateway {
        if gw.base_url.trim().is_empty() {
            anyhow::bail!(
                "profile '{}' gateway.base_url must not be empty",
                profile_name
            );
        }
        if gw.providers.is_empty() {
            anyhow::bail!(
                "profile '{}' gateway.providers must list at least one provider",
                profile_name
            );
        }
        let resolved_bearer = match gw.bearer.as_deref() {
            Some(value) => Some(resolve_secret_value(value)?),
            None => None,
        };
        resolved.gateway = Some(ResolvedGateway {
            base_url: gw.base_url.clone(),
            bearer: resolved_bearer,
            providers: gw.providers.clone(),
            endpoint_strip_v1_override: gw.endpoint_strip_v1,
            provider_headers: gw.provider_headers.clone(),
        });
    }

    Ok(resolved)
}

fn resolve_secret_value(value: &str) -> anyhow::Result<String> {
    if value.starts_with("secret://") {
        secrets::resolve_secret(value)
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> HmConfig {
        toml_edit::de::from_str(input).expect("config parses")
    }

    #[test]
    fn config_path_honors_xdg_config_home() {
        let base = std::env::temp_dir().join(format!("hm-config-test-{}", std::process::id()));
        let config_dir = base.join("xdg-config");
        let path = config_dir.join("hm/config.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "default_profile = \"proxy\"\n").unwrap();

        let resolved = config_path_from(Some(&config_dir), None);

        assert_eq!(resolved, path);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn config_path_does_not_fallback_when_xdg_config_home_is_explicit() {
        let base =
            std::env::temp_dir().join(format!("hm-config-test-missing-{}", std::process::id()));
        let config_dir = base.join("xdg-config");
        let home = base.join("home");
        let home_path = home.join(".config/hm/config.toml");
        std::fs::create_dir_all(home_path.parent().unwrap()).unwrap();
        std::fs::write(&home_path, "default_profile = \"real\"\n").unwrap();

        let resolved = config_path_from(Some(&config_dir), Some(&home));

        assert_eq!(resolved, config_dir.join("hm/config.toml"));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn gateway_block_parses_with_full_shape() {
        let config = parse(
            r#"
default_profile = "proxy"

[profiles.proxy]
description = "QA"

[profiles.proxy.gateway]
base_url = "https://gw.example/v1"
bearer = "literal-bearer"
providers = ["anthropic", "openai"]
endpoint_strip_v1 = true

[profiles.proxy.gateway.provider_headers.anthropic]
"x-api-key" = "{bearer}"
"#,
        );
        let resolved = resolve_profile(&config, Some("proxy")).unwrap();
        let gw = resolved.gateway.expect("gateway resolved");
        assert_eq!(gw.base_url, "https://gw.example/v1");
        assert_eq!(gw.bearer.as_deref(), Some("literal-bearer"));
        assert_eq!(gw.providers, vec!["anthropic", "openai"]);
        assert_eq!(gw.endpoint_strip_v1_override, Some(true));
        assert_eq!(
            gw.provider_headers
                .get("anthropic")
                .and_then(|m| m.get("x-api-key"))
                .map(String::as_str),
            Some("{bearer}")
        );
    }

    #[test]
    fn gateway_bearer_resolves_secret_file_uri() {
        let dir = std::env::temp_dir().join(format!("hm-gw-secret-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("key.txt");
        std::fs::write(&key_path, "super-secret-bearer\n").unwrap();
        let toml = format!(
            r#"
[profiles.proxy.gateway]
base_url = "https://gw.example/v1"
bearer = "secret://file://{}"
providers = ["anthropic"]
"#,
            key_path.display()
        );
        let config = parse(&toml);
        let resolved = resolve_profile(&config, Some("proxy")).unwrap();
        let gw = resolved.gateway.expect("gateway");
        assert_eq!(gw.bearer.as_deref(), Some("super-secret-bearer"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gateway_empty_providers_errors() {
        let config = parse(
            r#"
[profiles.proxy.gateway]
base_url = "https://gw.example/v1"
providers = []
"#,
        );
        let err = resolve_profile(&config, Some("proxy")).unwrap_err();
        assert!(
            err.to_string().contains("gateway.providers"),
            "expected providers error, got: {err:#}"
        );
    }

    #[test]
    fn gateway_empty_base_url_errors() {
        let config = parse(
            r#"
[profiles.proxy.gateway]
base_url = "  "
providers = ["anthropic"]
"#,
        );
        let err = resolve_profile(&config, Some("proxy")).unwrap_err();
        assert!(
            err.to_string().contains("gateway.base_url"),
            "expected base_url error, got: {err:#}"
        );
    }

    #[test]
    fn description_only_profile_still_resolves() {
        let config = parse(
            r#"
[profiles.bare]
description = "no llm no gateway no network"
"#,
        );
        let resolved = resolve_profile(&config, Some("bare")).unwrap();
        assert_eq!(
            resolved.description.as_deref(),
            Some("no llm no gateway no network")
        );
        assert!(resolved.gateway.is_none());
        assert!(resolved.bearer.is_none());
        assert!(resolved.endpoint.is_none());
    }

    #[test]
    fn llm_and_gateway_can_coexist() {
        let config = parse(
            r#"
[profiles.both.llm]
endpoint = "https://llm.example/v1"
bearer = "legacy-bearer"

[profiles.both.gateway]
base_url = "https://gw.example/v1"
bearer = "new-bearer"
providers = ["anthropic"]
"#,
        );
        let resolved = resolve_profile(&config, Some("both")).unwrap();
        assert_eq!(resolved.endpoint.as_deref(), Some("https://llm.example/v1"));
        assert_eq!(resolved.bearer.as_deref(), Some("legacy-bearer"));
        let gw = resolved.gateway.unwrap();
        assert_eq!(gw.base_url, "https://gw.example/v1");
        assert_eq!(gw.bearer.as_deref(), Some("new-bearer"));
    }
}
