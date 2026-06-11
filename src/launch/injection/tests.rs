use crate::config::{ResolvedGateway, ResolvedProfile};
use crate::runtimes::manifest::{
    CodexConfigSeedInjection, EnvInjection, ProviderConfigSeedInjection,
};
use std::collections::{BTreeMap, HashMap};

mod codex_errors;
mod codex_preview;
mod codex_write;
mod env;
mod provider_config;
mod provider_config_errors;

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
        provider_api_key_envs: provider_api_key_envs(&[
            ("anthropic", "ANTHROPIC_API_KEY"),
            ("openai", "OPENAI_API_KEY"),
            ("google", "GOOGLE_API_KEY"),
        ]),
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
        provider_api_key_envs: provider_api_key_envs(&[
            ("anthropic", "ANTHROPIC_API_KEY"),
            ("openai", "OPENAI_API_KEY"),
            ("google", "GOOGLE_API_KEY"),
        ]),
        overwrite: false,
        endpoint_strip_v1: false,
        provider_header_overrides: headers,
        legacy_provider: None,
    }
}

fn provider_api_key_envs(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(provider, env)| ((*provider).to_string(), (*env).to_string()))
        .collect()
}

fn codex_config_seed_injection() -> CodexConfigSeedInjection {
    CodexConfigSeedInjection {
        config_path: "{home}/.codex/config.toml".to_string(),
        openai_base_url_key: "openai_base_url".to_string(),
        model_provider_key: "model_provider".to_string(),
        model_provider_value: "openai".to_string(),
        provider: "openai".to_string(),
        supported_providers: vec!["openai".to_string()],
        api_key_env: "CODEX_API_KEY".to_string(),
        strip_envs: vec![
            "OPENAI_API_KEY".to_string(),
            "OPENAI_BASE_URL".to_string(),
            "CODEX_API_KEY".to_string(),
            "CODEX_ACCESS_TOKEN".to_string(),
        ],
        overwrite: false,
        endpoint_strip_v1: false,
    }
}
