use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

use crate::config::ResolvedProfile;
use crate::isolation::ensure_safe_write_path;
use crate::runtimes::manifest::{EnvInjection, InjectionRecord, ProviderConfigSeedInjection};

pub fn apply_injection(
    injection: &InjectionRecord,
    resolved: &ResolvedProfile,
    env: &mut HashMap<String, String>,
    home_dir: &Path,
) -> Result<Option<PathBuf>> {
    match injection {
        InjectionRecord::Env(spec) => {
            apply_env_strategy(spec, resolved, env)?;
            Ok(None)
        }
        InjectionRecord::ProviderConfigSeed(spec) => {
            let path = apply_provider_config_seed_strategy(spec, resolved, home_dir)?;
            Ok(Some(path))
        }
    }
}

pub fn apply_env_strategy(
    spec: &EnvInjection,
    resolved: &ResolvedProfile,
    env: &mut HashMap<String, String>,
) -> Result<()> {
    for key in &spec.strip_envs {
        env.remove(key);
    }

    if let Some(gateway) = resolved.gateway.as_ref() {
        if !gateway.providers.iter().any(|p| p == &spec.provider) {
            anyhow::bail!(
                "profile gateway routes providers [{}] but runtime needs provider '{}' (supported by runtime: [{}])",
                gateway.providers.join(", "),
                spec.provider,
                spec.supported_providers.join(", ")
            );
        }
        if !spec.supported_providers.contains(&spec.provider) {
            anyhow::bail!(
                "runtime provider '{}' not declared in supported_providers [{}]",
                spec.provider,
                spec.supported_providers.join(", ")
            );
        }
        let effective_strip = gateway
            .endpoint_strip_v1_override
            .unwrap_or(spec.endpoint_strip_v1);
        let endpoint = effective_endpoint(&gateway.base_url, effective_strip);
        if let Some(bearer) = gateway.bearer.as_deref() {
            validate_bearer_value_at_runtime(bearer)?;
        }
        env.insert(spec.endpoint_env.clone(), endpoint);
        if let Some(bearer) = gateway.bearer.as_deref() {
            env.insert(spec.api_key_env.clone(), bearer.to_string());
        }
        return Ok(());
    }

    if let Some(bearer) = resolved.bearer.as_deref() {
        validate_bearer_value_at_runtime(bearer)?;
    }
    if let Some(endpoint) = resolved.endpoint.as_deref() {
        let trimmed = effective_endpoint(endpoint, spec.endpoint_strip_v1);
        env.insert(spec.endpoint_env.clone(), trimmed);
    }
    if let Some(bearer) = resolved.bearer.as_deref() {
        env.insert(spec.api_key_env.clone(), bearer.to_string());
    }
    Ok(())
}

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

    let profile_provider_headers = resolved.gateway.as_ref().map(|gw| &gw.provider_headers);

    for provider in &providers {
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
    fs::write(&config_path, format!("{pretty}\n"))
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(config_path)
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

fn validate_header_name_at_runtime(name: &str) -> Result<()> {
    if name.is_empty()
        || name
            .bytes()
            .any(|b| !b.is_ascii_graphic() || b == b':' || b.is_ascii_control())
    {
        anyhow::bail!(
            "gateway provider_headers: invalid header name '{}' (must be printable ASCII, no ':' or control chars)",
            name
        );
    }
    Ok(())
}

fn validate_bearer_value_at_runtime(value: &str) -> Result<()> {
    if value
        .chars()
        .any(|ch| ch == '\r' || ch == '\n' || ch == '\0')
    {
        anyhow::bail!("bearer contains CRLF/NUL (refused to prevent header injection)");
    }
    if value.chars().any(char::is_control) {
        anyhow::bail!("bearer contains control characters");
    }
    Ok(())
}

fn validate_header_value_at_runtime(name: &str, value: &str) -> Result<()> {
    if value
        .chars()
        .any(|ch| ch == '\r' || ch == '\n' || ch == '\0')
    {
        anyhow::bail!(
            "gateway provider_headers: header '{}' value contains CRLF/NUL (refused to prevent header injection)",
            name
        );
    }
    if value.chars().any(char::is_control) {
        anyhow::bail!(
            "gateway provider_headers: header '{}' value contains control characters",
            name
        );
    }
    Ok(())
}

fn render_header_value_at_runtime(name: &str, template: &str, bearer: &str) -> Result<String> {
    validate_header_value_at_runtime(name, template)?;
    let value = template.replace("{bearer}", bearer);
    validate_header_value_at_runtime(name, &value)?;
    Ok(value)
}

fn effective_endpoint(base_url: &str, strip_v1: bool) -> String {
    if !strip_v1 {
        return base_url.to_string();
    }
    base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .to_string()
}

fn expand_home_template(template: &str, home_dir: &Path) -> PathBuf {
    let home_str = home_dir.display().to_string();
    PathBuf::from(template.replace("{home}", &home_str))
}

fn set_dotted_path(node: &mut Value, dotted: &str, leaf: Value) {
    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    let map = node.as_object_mut().expect("object enforced above");
    let (head, rest) = match dotted.split_once('.') {
        Some((h, r)) => (h, Some(r)),
        None => (dotted, None),
    };
    if head.is_empty() {
        return;
    }
    match rest {
        None => {
            map.insert(head.to_string(), leaf);
        }
        Some(rest) => {
            let next = map
                .entry(head.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            set_dotted_path(next, rest, leaf);
        }
    }
}

fn merge_provider(root: &mut Value, root_key: &str, provider: &str, value: Value) {
    if !root.is_object() {
        *root = Value::Object(Map::new());
    }
    let container = walk_or_create(root, root_key);
    if !container.is_object() {
        *container = Value::Object(Map::new());
    }
    let map = container.as_object_mut().expect("object enforced above");
    match map.get_mut(provider) {
        Some(existing) if existing.is_object() && value.is_object() => deep_merge(existing, value),
        _ => {
            map.insert(provider.to_string(), value);
        }
    }
}

fn walk_or_create<'a>(node: &'a mut Value, dotted: &str) -> &'a mut Value {
    let (head, rest) = match dotted.split_once('.') {
        Some((h, r)) => (h, Some(r)),
        None => (dotted, None),
    };
    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    let map = node.as_object_mut().expect("object enforced above");
    let next = map
        .entry(head.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    match rest {
        None => next,
        Some(rest) => walk_or_create(next, rest),
    }
}

fn deep_merge(target: &mut Value, source: Value) {
    if let (Some(t_map), Value::Object(s_map)) = (target.as_object_mut(), source.clone()) {
        for (k, v) in s_map {
            match t_map.get_mut(&k) {
                Some(existing) if existing.is_object() && v.is_object() => {
                    deep_merge(existing, v);
                }
                _ => {
                    t_map.insert(k, v);
                }
            }
        }
    } else {
        *target = source;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedGateway;
    use crate::runtimes::manifest::EnvInjection;
    use std::collections::BTreeMap;

    fn empty_profile(name: &str) -> ResolvedProfile {
        ResolvedProfile {
            name: name.to_string(),
            description: None,
            http_proxy: None,
            https_proxy: None,
            no_proxy: None,
            endpoint: None,
            bearer: None,
            gateway: None,
        }
    }

    fn proxy_profile_with_gateway(providers: Vec<&str>, bearer: &str) -> ResolvedProfile {
        let mut p = empty_profile("proxy");
        p.gateway = Some(ResolvedGateway {
            base_url: "https://gw.example/v1".to_string(),
            bearer: Some(bearer.to_string()),
            providers: providers.into_iter().map(String::from).collect(),
            endpoint_strip_v1_override: None,
            provider_headers: HashMap::new(),
        });
        p
    }

    fn claude_env_injection() -> EnvInjection {
        EnvInjection {
            provider: "anthropic".to_string(),
            supported_providers: vec!["anthropic".to_string()],
            endpoint_env: "ANTHROPIC_BASE_URL".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            strip_envs: vec![
                "ANTHROPIC_API_KEY".to_string(),
                "ANTHROPIC_BASE_URL".to_string(),
            ],
            endpoint_strip_v1: true,
        }
    }

    fn codex_env_injection() -> EnvInjection {
        EnvInjection {
            provider: "openai".to_string(),
            supported_providers: vec!["openai".to_string()],
            endpoint_env: "OPENAI_BASE_URL".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            strip_envs: vec!["OPENAI_API_KEY".to_string()],
            endpoint_strip_v1: false,
        }
    }

    fn opencode_seed_injection() -> ProviderConfigSeedInjection {
        let mut headers = BTreeMap::new();
        let mut anthropic = BTreeMap::new();
        anthropic.insert("x-api-key".to_string(), "{bearer}".to_string());
        anthropic.insert("Authorization".to_string(), "Bearer {bearer}".to_string());
        headers.insert("anthropic".to_string(), anthropic);
        ProviderConfigSeedInjection {
            config_path: "{home}/.config/opencode/opencode.json".to_string(),
            root_key: "provider".to_string(),
            provider_base_url_key: "options.baseURL".to_string(),
            provider_api_key_key: "options.apiKey".to_string(),
            provider_headers_key: Some("options.headers".to_string()),
            supported_providers: vec![
                "anthropic".to_string(),
                "openai".to_string(),
                "google".to_string(),
            ],
            overwrite: false,
            endpoint_strip_v1: false,
            provider_header_overrides: headers,
            legacy_provider: Some("openai".to_string()),
        }
    }

    fn pi_seed_injection() -> ProviderConfigSeedInjection {
        let mut headers = BTreeMap::new();
        let mut anthropic = BTreeMap::new();
        anthropic.insert("x-api-key".to_string(), "{bearer}".to_string());
        headers.insert("anthropic".to_string(), anthropic);
        ProviderConfigSeedInjection {
            config_path: "{home}/.pi/agent/models.json".to_string(),
            root_key: "providers".to_string(),
            provider_base_url_key: "baseUrl".to_string(),
            provider_api_key_key: "apiKey".to_string(),
            provider_headers_key: Some("headers".to_string()),
            supported_providers: vec![
                "anthropic".to_string(),
                "openai".to_string(),
                "google".to_string(),
            ],
            overwrite: false,
            endpoint_strip_v1: false,
            provider_header_overrides: headers,
            legacy_provider: None,
        }
    }

    #[test]
    fn env_strategy_strips_envs_and_injects_from_gateway_for_claude() {
        let mut env: HashMap<String, String> = HashMap::new();
        env.insert("ANTHROPIC_API_KEY".to_string(), "host-key".to_string());
        env.insert("ANTHROPIC_BASE_URL".to_string(), "host-url".to_string());
        env.insert("UNRELATED".to_string(), "keep".to_string());
        let resolved = proxy_profile_with_gateway(vec!["anthropic"], "live-bearer");

        apply_env_strategy(&claude_env_injection(), &resolved, &mut env).unwrap();

        assert_eq!(
            env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://gw.example")
        );
        assert_eq!(
            env.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some("live-bearer")
        );
        assert_eq!(env.get("UNRELATED").map(String::as_str), Some("keep"));
    }

    #[test]
    fn env_strategy_keeps_v1_for_codex() {
        let mut env: HashMap<String, String> = HashMap::new();
        let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-codex");

        apply_env_strategy(&codex_env_injection(), &resolved, &mut env).unwrap();

        assert_eq!(
            env.get("OPENAI_BASE_URL").map(String::as_str),
            Some("https://gw.example/v1")
        );
        assert_eq!(
            env.get("OPENAI_API_KEY").map(String::as_str),
            Some("bearer-codex")
        );
    }

    #[test]
    fn env_strategy_errors_when_gateway_misses_runtime_provider() {
        let mut env: HashMap<String, String> = HashMap::new();
        let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer");

        let err = apply_env_strategy(&claude_env_injection(), &resolved, &mut env).unwrap_err();
        assert!(
            err.to_string().contains("anthropic"),
            "expected mismatch mentioning anthropic: {err:#}"
        );
        assert!(
            err.to_string().contains("supported by runtime"),
            "expected supported list: {err:#}"
        );
    }

    #[test]
    fn env_strategy_legacy_llm_path_when_no_gateway() {
        let mut env: HashMap<String, String> = HashMap::new();
        let mut p = empty_profile("legacy");
        p.endpoint = Some("https://llm.example/v1".to_string());
        p.bearer = Some("legacy-key".to_string());

        apply_env_strategy(&claude_env_injection(), &p, &mut env).unwrap();

        assert_eq!(
            env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://llm.example")
        );
        assert_eq!(
            env.get("ANTHROPIC_API_KEY").map(String::as_str),
            Some("legacy-key")
        );
    }

    #[test]
    fn env_strategy_endpoint_strip_v1_override_from_gateway() {
        let mut env: HashMap<String, String> = HashMap::new();
        let mut resolved = proxy_profile_with_gateway(vec!["anthropic"], "bearer");
        resolved
            .gateway
            .as_mut()
            .unwrap()
            .endpoint_strip_v1_override = Some(false);

        apply_env_strategy(&claude_env_injection(), &resolved, &mut env).unwrap();

        assert_eq!(
            env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://gw.example/v1")
        );
    }

    #[test]
    fn env_strategy_rejects_gateway_bearer_crlf_before_inserting_api_key_env() {
        let mut env: HashMap<String, String> = HashMap::new();
        let resolved = proxy_profile_with_gateway(vec!["anthropic"], "good\r\nX-Injected: evil");

        let err = apply_env_strategy(&claude_env_injection(), &resolved, &mut env).unwrap_err();

        assert!(
            err.to_string().contains("bearer contains CRLF/NUL"),
            "expected bearer CRLF rejection, got: {err:#}"
        );
        assert!(
            !env.contains_key("ANTHROPIC_API_KEY"),
            "unsafe bearer must not be inserted into API-key env"
        );
        assert!(
            !env.contains_key("ANTHROPIC_BASE_URL"),
            "env strategy should fail before partial endpoint insertion"
        );
    }

    #[test]
    fn env_strategy_rejects_legacy_bearer_nul_before_inserting_api_key_env() {
        let mut env: HashMap<String, String> = HashMap::new();
        let mut p = empty_profile("legacy");
        p.endpoint = Some("https://llm.example/v1".to_string());
        p.bearer = Some("bad\0bearer".to_string());

        let err = apply_env_strategy(&claude_env_injection(), &p, &mut env).unwrap_err();

        assert!(
            err.to_string().contains("bearer contains CRLF/NUL"),
            "expected bearer NUL rejection, got: {err:#}"
        );
        assert!(!env.contains_key("ANTHROPIC_API_KEY"));
        assert!(!env.contains_key("ANTHROPIC_BASE_URL"));
    }

    #[test]
    fn provider_config_seed_writes_opencode_json_for_three_providers() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let resolved =
            proxy_profile_with_gateway(vec!["anthropic", "openai", "google"], "live-bearer-aaaa");

        let path = apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, home)
            .expect("seed writes");

        assert_eq!(path, home.join(".config/opencode/opencode.json"));
        let body: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        for provider in ["anthropic", "openai", "google"] {
            let p = &body["provider"][provider];
            assert_eq!(
                p["options"]["baseURL"].as_str(),
                Some("https://gw.example/v1")
            );
            assert_eq!(p["options"]["apiKey"].as_str(), Some("live-bearer-aaaa"));
        }
        let anthropic_headers = &body["provider"]["anthropic"]["options"]["headers"];
        assert_eq!(
            anthropic_headers["x-api-key"].as_str(),
            Some("live-bearer-aaaa")
        );
        assert_eq!(
            anthropic_headers["Authorization"].as_str(),
            Some("Bearer live-bearer-aaaa")
        );
        // openai/google should NOT have anthropic-specific headers
        assert!(body["provider"]["openai"]["options"]
            .get("headers")
            .map(|h| !h.as_object().unwrap().contains_key("x-api-key"))
            .unwrap_or(true));
    }

    #[test]
    fn provider_config_seed_writes_pi_models_json_for_three_providers() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let resolved =
            proxy_profile_with_gateway(vec!["anthropic", "openai", "google"], "pi-bearer");

        let path =
            apply_provider_config_seed_strategy(&pi_seed_injection(), &resolved, home).unwrap();

        assert_eq!(path, home.join(".pi/agent/models.json"));
        let body: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        for provider in ["anthropic", "openai", "google"] {
            let p = &body["providers"][provider];
            assert_eq!(p["baseUrl"].as_str(), Some("https://gw.example/v1"));
            assert_eq!(p["apiKey"].as_str(), Some("pi-bearer"));
        }
        assert_eq!(
            body["providers"]["anthropic"]["headers"]["x-api-key"].as_str(),
            Some("pi-bearer")
        );
    }

    #[test]
    fn provider_config_seed_preserves_existing_unrelated_provider() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let target = home.join(".config/opencode/opencode.json");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(
            &target,
            r#"{ "provider": { "custom": { "options": { "baseURL": "https://custom" } } } }"#,
        )
        .unwrap();

        let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer");
        apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, home).unwrap();

        let body: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(
            body["provider"]["custom"]["options"]["baseURL"].as_str(),
            Some("https://custom")
        );
        assert_eq!(
            body["provider"]["openai"]["options"]["baseURL"].as_str(),
            Some("https://gw.example/v1")
        );
    }

    #[test]
    fn provider_config_seed_errors_on_unknown_provider() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let resolved = proxy_profile_with_gateway(vec!["mystery"], "bearer");

        let err = apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, home)
            .unwrap_err();
        assert!(
            err.to_string().contains("mystery"),
            "expected mystery in error: {err:#}"
        );
        assert!(
            err.to_string().contains("Supported"),
            "expected supported list in error: {err:#}"
        );
    }

    #[test]
    fn provider_config_seed_errors_when_no_gateway_and_no_legacy_llm() {
        let tmp = tempfile::tempdir().unwrap();
        let p = empty_profile("no-gw");
        let err = apply_provider_config_seed_strategy(&opencode_seed_injection(), &p, tmp.path())
            .unwrap_err();
        assert!(
            err.to_string().contains("gateway"),
            "expected gateway error: {err:#}"
        );
    }

    #[test]
    fn provider_config_seed_legacy_llm_seeds_legacy_provider_only_for_opencode() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let mut p = empty_profile("legacy");
        p.endpoint = Some("https://legacy.example/v1".to_string());
        p.bearer = Some("legacy-bearer-aaa".to_string());

        let path =
            apply_provider_config_seed_strategy(&opencode_seed_injection(), &p, home).unwrap();

        let body: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            body["provider"]["openai"]["options"]["baseURL"].as_str(),
            Some("https://legacy.example/v1")
        );
        assert_eq!(
            body["provider"]["openai"]["options"]["apiKey"].as_str(),
            Some("legacy-bearer-aaa")
        );
        assert!(
            body["provider"]["anthropic"].is_null(),
            "legacy llm must seed only the declared legacy_provider, not anthropic"
        );
        assert!(
            body["provider"]["google"].is_null(),
            "legacy llm must seed only the declared legacy_provider, not google"
        );
    }

    #[test]
    fn provider_config_seed_legacy_llm_errors_when_runtime_has_no_legacy_provider() {
        let tmp = tempfile::tempdir().unwrap();
        let mut p = empty_profile("legacy");
        p.endpoint = Some("https://legacy.example/v1".to_string());
        p.bearer = Some("legacy-bearer".to_string());

        let err =
            apply_provider_config_seed_strategy(&pi_seed_injection(), &p, tmp.path()).unwrap_err();
        assert!(
            err.to_string().contains("no legacy_provider"),
            "expected no-legacy-provider error: {err:#}"
        );
    }

    #[test]
    fn validate_seed_reports_legacy_llm_source() {
        let mut p = empty_profile("legacy");
        p.endpoint = Some("https://legacy.example/v1".to_string());
        p.bearer = Some("legacy-bearer".to_string());

        let preview = validate_provider_config_seed(&opencode_seed_injection(), &p).unwrap();
        assert_eq!(preview.source, ProviderConfigSeedSource::LegacyLlm);
        assert_eq!(preview.providers, vec!["openai".to_string()]);
        assert_eq!(preview.endpoint, "https://legacy.example/v1");
    }

    #[test]
    fn provider_config_seed_rejects_bearer_crlf_even_without_header_overrides() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved = proxy_profile_with_gateway(vec!["openai"], "good\r\nX-Injected: evil");

        let err =
            apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, tmp.path())
                .unwrap_err();

        assert!(
            err.to_string().contains("bearer contains CRLF/NUL"),
            "expected bearer CRLF rejection, got: {err:#}"
        );
        assert!(
            !tmp.path().join(".config/opencode/opencode.json").exists(),
            "seed file must not be written when bearer is unsafe"
        );
    }

    #[test]
    fn validate_seed_rejects_bearer_nul_even_without_header_overrides() {
        let resolved = proxy_profile_with_gateway(vec!["openai"], "bad\0bearer");

        let err = validate_provider_config_seed(&opencode_seed_injection(), &resolved).unwrap_err();

        assert!(
            err.to_string().contains("bearer contains CRLF/NUL"),
            "dry-run must reject unsafe bearer, got: {err:#}"
        );
    }

    #[test]
    fn provider_config_seed_rejects_bearer_with_embedded_crlf() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved = proxy_profile_with_gateway(vec!["anthropic"], "good\r\nX-Injected: evil");

        let err =
            apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, tmp.path())
                .unwrap_err();
        assert!(
            err.to_string().contains("CRLF/NUL"),
            "expected CRLF rejection on bearer substitution, got: {err:#}"
        );
        assert!(
            !tmp.path().join(".config/opencode/opencode.json").exists(),
            "seed file must not be written when bearer is unsafe"
        );
    }

    #[test]
    fn provider_config_seed_rejects_bearer_with_null_byte() {
        let tmp = tempfile::tempdir().unwrap();
        let resolved = proxy_profile_with_gateway(vec!["anthropic"], "leading\0nul");

        let err =
            apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, tmp.path())
                .unwrap_err();
        assert!(
            err.to_string().contains("CRLF/NUL"),
            "expected NUL rejection on bearer substitution, got: {err:#}"
        );
        assert!(
            !tmp.path().join(".config/opencode/opencode.json").exists(),
            "seed file must not be written when bearer is unsafe"
        );
    }
}
