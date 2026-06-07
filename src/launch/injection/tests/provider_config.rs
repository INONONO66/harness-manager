use super::*;
use crate::launch::injection::{
    apply_provider_config_seed_strategy, validate_provider_config_seed, ProviderConfigSeedSource,
};
use serde_json::Value;
use std::fs;

#[test]
fn provider_config_seed_writes_opencode_json_for_three_providers() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let mut env = HashMap::new();
    let resolved =
        proxy_profile_with_gateway(vec!["anthropic", "openai", "google"], "live-bearer-aaaa");

    let path =
        apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, &mut env, home)
            .expect("seed writes");
    assert_eq!(
        env.get("ANTHROPIC_API_KEY"),
        Some(&"live-bearer-aaaa".to_string())
    );
    assert_eq!(
        env.get("OPENAI_API_KEY"),
        Some(&"live-bearer-aaaa".to_string())
    );
    assert_eq!(
        env.get("GOOGLE_API_KEY"),
        Some(&"live-bearer-aaaa".to_string())
    );

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
    let mut env = HashMap::new();
    let resolved = proxy_profile_with_gateway(vec!["anthropic", "openai", "google"], "pi-bearer");

    let path = apply_provider_config_seed_strategy(&pi_seed_injection(), &resolved, &mut env, home)
        .unwrap();

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
    let mut env = HashMap::new();
    apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, &mut env, home)
        .unwrap();

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
    let mut env = HashMap::new();

    let err =
        apply_provider_config_seed_strategy(&opencode_seed_injection(), &resolved, &mut env, home)
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
    let mut env = HashMap::new();
    let err =
        apply_provider_config_seed_strategy(&opencode_seed_injection(), &p, &mut env, tmp.path())
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
    let mut env = HashMap::new();

    let path = apply_provider_config_seed_strategy(&opencode_seed_injection(), &p, &mut env, home)
        .unwrap();

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
    let mut env = HashMap::new();

    let err = apply_provider_config_seed_strategy(&pi_seed_injection(), &p, &mut env, tmp.path())
        .unwrap_err();
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
