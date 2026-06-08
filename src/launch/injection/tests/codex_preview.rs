use super::*;
use crate::launch::injection::{
    apply_codex_config_seed_strategy, validate_codex_config_seed, ProviderConfigSeedSource,
};
use std::{collections::HashMap, fs};

#[test]
fn codex_seed_endpoint_strip_v1_true_drops_v1_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let mut spec = codex_config_seed_injection();
    spec.endpoint_strip_v1 = true;
    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-test");
    let mut env: HashMap<String, String> = HashMap::new();

    let path = apply_codex_config_seed_strategy(&spec, &resolved, &mut env, home).unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    assert!(
        contents.contains(r#"openai_base_url = "https://gw.example""#),
        "expected /v1 stripped: {contents}"
    );
}

#[test]
fn codex_seed_legacy_llm_path_works_with_single_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let mut p = empty_profile("legacy");
    p.endpoint = Some("https://legacy.example/v1".to_string());
    p.bearer = Some("legacy-codex-bearer".to_string());
    let mut env: HashMap<String, String> = HashMap::new();

    let path = apply_codex_config_seed_strategy(&codex_config_seed_injection(), &p, &mut env, home)
        .unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    assert!(
        contents.contains(r#"openai_base_url = "https://legacy.example/v1""#),
        "expected legacy endpoint: {contents}"
    );
    assert!(
        contents.contains(r#"model_provider = "openai""#),
        "expected model_provider: {contents}"
    );
    assert_eq!(
        env.get("CODEX_API_KEY").map(String::as_str),
        Some("legacy-codex-bearer"),
        "CODEX_API_KEY must be set from legacy bearer"
    );
}

#[test]
fn codex_seed_does_not_write_bearer_to_config_file() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let unique_bearer = "qa-distinct-bearer-7K9mN2pV5xL3wQ8jR4";
    let resolved = proxy_profile_with_gateway(vec!["openai"], unique_bearer);
    let mut env: HashMap<String, String> = HashMap::new();

    let path =
        apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
            .unwrap();

    let contents = fs::read_to_string(&path).unwrap();
    assert!(
        !contents.contains(unique_bearer),
        "bearer must NEVER appear in the written config.toml, but file contains it: {contents}"
    );
    assert_eq!(
        env.get("CODEX_API_KEY").map(String::as_str),
        Some(unique_bearer),
        "bearer must reach env, not the file"
    );
}

#[test]
fn validate_codex_config_seed_reports_top_level_writes() {
    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-abc");
    let preview = validate_codex_config_seed(&codex_config_seed_injection(), &resolved).unwrap();

    assert_eq!(preview.provider, "openai");
    assert_eq!(preview.endpoint, "https://gw.example/v1");
    assert_eq!(preview.source, ProviderConfigSeedSource::Gateway);
    assert_eq!(preview.api_key_env, "CODEX_API_KEY");
    assert!(preview.config_path_display.contains(".codex/config.toml"));
    assert_eq!(preview.top_level_writes.len(), 2);
    let writes: BTreeMap<_, _> = preview.top_level_writes.iter().cloned().collect();
    assert_eq!(
        writes.get("openai_base_url").map(String::as_str),
        Some("https://gw.example/v1")
    );
    assert_eq!(
        writes.get("model_provider").map(String::as_str),
        Some("openai")
    );
}
