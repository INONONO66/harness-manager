use super::*;
use crate::launch::injection::apply_codex_config_seed_strategy;
use std::{collections::HashMap, fs};

#[test]
fn codex_seed_creates_minimal_config_toml_with_top_level_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let resolved = proxy_profile_with_gateway(vec!["openai"], "codex-bearer-abcd");
    let mut env: HashMap<String, String> = HashMap::new();

    let path =
        apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
            .expect("seed writes");

    assert_eq!(path, home.join(".codex/config.toml"));
    let contents = fs::read_to_string(&path).unwrap();
    assert!(
        contents.contains(r#"openai_base_url = "https://gw.example/v1""#),
        "missing openai_base_url top-level key: {contents}"
    );
    assert!(
        contents.contains(r#"model_provider = "openai""#),
        "missing model_provider top-level key: {contents}"
    );
}

#[test]
fn codex_seed_sets_codex_api_key_env_and_strips_openai_envs() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let resolved = proxy_profile_with_gateway(vec!["openai"], "codex-bearer-abcd");
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), "host-key".to_string());
    env.insert("OPENAI_BASE_URL".to_string(), "host-url".to_string());
    env.insert("CODEX_API_KEY".to_string(), "host-codex-key".to_string());
    env.insert("CODEX_ACCESS_TOKEN".to_string(), "host-token".to_string());
    env.insert("UNRELATED".to_string(), "keep".to_string());

    apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
        .unwrap();

    assert!(
        !env.contains_key("OPENAI_API_KEY"),
        "OPENAI_API_KEY must be stripped"
    );
    assert!(
        !env.contains_key("OPENAI_BASE_URL"),
        "OPENAI_BASE_URL must be stripped"
    );
    assert_eq!(
        env.get("CODEX_API_KEY").map(String::as_str),
        Some("codex-bearer-abcd"),
        "CODEX_API_KEY must be reset to bearer"
    );
    assert!(
        !env.contains_key("CODEX_ACCESS_TOKEN"),
        "CODEX_ACCESS_TOKEN must be stripped"
    );
    assert_eq!(env.get("UNRELATED").map(String::as_str), Some("keep"));
}

#[test]
fn codex_seed_preserves_existing_seed_file_stanzas() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let target = home.join(".codex/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        r#"analytics_enabled = false
check_for_update_on_startup = false
cli_auth_credentials_store = "file"
mcp_oauth_credentials_store = "file"
"#,
    )
    .unwrap();

    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-test");
    let mut env: HashMap<String, String> = HashMap::new();
    apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
        .unwrap();

    let contents = fs::read_to_string(&target).unwrap();
    for key in [
        "analytics_enabled = false",
        "check_for_update_on_startup = false",
        r#"cli_auth_credentials_store = "file""#,
        r#"mcp_oauth_credentials_store = "file""#,
        r#"openai_base_url = "https://gw.example/v1""#,
        r#"model_provider = "openai""#,
    ] {
        assert!(
            contents.contains(key),
            "missing key '{key}' in merged config: {contents}"
        );
    }
}

#[test]
fn codex_seed_preserves_existing_user_top_level_keys_and_table() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let target = home.join(".codex/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        r#"model = "gpt-5"

[history]
persistence = "save-all"
"#,
    )
    .unwrap();

    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-test");
    let mut env: HashMap<String, String> = HashMap::new();
    apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
        .unwrap();

    let contents = fs::read_to_string(&target).unwrap();
    assert!(
        contents.contains(r#"model = "gpt-5""#),
        "lost user `model` top-level: {contents}"
    );
    assert!(
        contents.contains("[history]"),
        "lost user [history] table: {contents}"
    );
    assert!(
        contents.contains(r#"persistence = "save-all""#),
        "lost history.persistence: {contents}"
    );
    assert!(
        contents.contains(r#"openai_base_url = "https://gw.example/v1""#),
        "missing openai_base_url: {contents}"
    );
    assert!(
        contents.contains(r#"model_provider = "openai""#),
        "missing model_provider: {contents}"
    );
}

#[test]
fn codex_seed_preserves_existing_toml_comments_and_key_order() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let target = home.join(".codex/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        r#"# User comment preserved across hm injection
analytics_enabled = false
# Another comment
check_for_update_on_startup = false
"#,
    )
    .unwrap();

    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-test");
    let mut env: HashMap<String, String> = HashMap::new();
    apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
        .unwrap();

    let contents = fs::read_to_string(&target).unwrap();
    assert!(
        contents.contains("# User comment preserved across hm injection"),
        "lost user comment 1: {contents}"
    );
    assert!(
        contents.contains("# Another comment"),
        "lost user comment 2: {contents}"
    );
}
