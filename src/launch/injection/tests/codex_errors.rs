use super::*;
use crate::launch::injection::apply_codex_config_seed_strategy;
use std::{collections::HashMap, fs};

#[test]
fn codex_seed_overwrites_existing_model_provider_key() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let target = home.join(".codex/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, "model_provider = \"anthropic\"\n").unwrap();

    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-test");
    let mut env: HashMap<String, String> = HashMap::new();
    apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
        .unwrap();

    let contents = fs::read_to_string(&target).unwrap();
    assert!(
        contents.contains(r#"model_provider = "hm-proxy-openai""#),
        "model_provider must be overwritten to hm proxy provider: {contents}"
    );
    assert!(
        !contents.contains(r#"model_provider = "anthropic""#),
        "previous anthropic value must be replaced: {contents}"
    );
}

#[test]
fn codex_seed_errors_when_no_gateway_and_no_legacy_endpoint() {
    let tmp = tempfile::tempdir().unwrap();
    let p = empty_profile("no-gw");
    let mut env: HashMap<String, String> = HashMap::new();

    let err =
        apply_codex_config_seed_strategy(&codex_config_seed_injection(), &p, &mut env, tmp.path())
            .unwrap_err();

    assert!(
        err.to_string().contains("gateway"),
        "expected gateway error: {err:#}"
    );
    assert!(
        !tmp.path().join(".codex/config.toml").exists(),
        "no file should be written"
    );
    assert!(env.is_empty(), "no env mutation expected");
}

#[test]
fn codex_seed_errors_when_gateway_provider_mismatch() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = proxy_profile_with_gateway(vec!["anthropic"], "bearer");
    let mut env: HashMap<String, String> = HashMap::new();

    let err = apply_codex_config_seed_strategy(
        &codex_config_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("openai"),
        "expected openai-mismatch error: {err:#}"
    );
    assert!(
        !tmp.path().join(".codex/config.toml").exists(),
        "no file should be written"
    );
    assert!(env.is_empty(), "no env mutation expected");
}

#[test]
fn codex_seed_rejects_empty_bearer_before_writing_file_or_env() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = proxy_profile_with_gateway(vec!["openai"], "");
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), "preserve-me".to_string());

    let err = apply_codex_config_seed_strategy(
        &codex_config_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("bearer is empty or whitespace"),
        "expected empty bearer rejection, got: {err:#}"
    );
    assert!(
        !tmp.path().join(".codex/config.toml").exists(),
        "file MUST NOT be written when bearer is empty"
    );
    assert_eq!(
        env.get("OPENAI_API_KEY").map(String::as_str),
        Some("preserve-me"),
        "OPENAI_API_KEY must remain (strip did not execute)"
    );
    assert!(
        !env.contains_key("CODEX_API_KEY"),
        "CODEX_API_KEY must not be set with empty bearer"
    );
}

#[test]
fn codex_seed_rejects_whitespace_bearer() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = proxy_profile_with_gateway(vec!["openai"], " \t\n ");
    let mut env: HashMap<String, String> = HashMap::new();

    let err = apply_codex_config_seed_strategy(
        &codex_config_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("bearer is empty or whitespace"),
        "expected whitespace bearer rejection, got: {err:#}"
    );
    assert!(env.is_empty(), "no env mutation expected");
}

#[test]
fn codex_seed_rejects_bearer_crlf_before_writing_file_or_env() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = proxy_profile_with_gateway(vec!["openai"], "good\r\nX-Evil: injected");
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), "preserve-me".to_string());
    env.insert("OPENAI_BASE_URL".to_string(), "preserve-me-too".to_string());

    let err = apply_codex_config_seed_strategy(
        &codex_config_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("bearer contains CRLF/NUL"),
        "expected CRLF rejection: {err:#}"
    );
    assert!(
        !tmp.path().join(".codex/config.toml").exists(),
        "file MUST NOT be written on bearer error"
    );
    assert_eq!(
        env.get("OPENAI_API_KEY").map(String::as_str),
        Some("preserve-me"),
        "OPENAI_API_KEY must remain (strip did not execute)"
    );
    assert_eq!(
        env.get("OPENAI_BASE_URL").map(String::as_str),
        Some("preserve-me-too"),
        "OPENAI_BASE_URL must remain (strip did not execute)"
    );
    assert!(
        !env.contains_key("CODEX_API_KEY"),
        "CODEX_API_KEY must not be set with unsafe bearer"
    );
}

#[test]
fn codex_seed_rejects_bearer_null_byte_before_writing_file_or_env() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = proxy_profile_with_gateway(vec!["openai"], "bad\0bearer");
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), "preserve-me".to_string());

    let err = apply_codex_config_seed_strategy(
        &codex_config_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("bearer contains CRLF/NUL"),
        "expected NUL rejection: {err:#}"
    );
    assert!(
        !tmp.path().join(".codex/config.toml").exists(),
        "file MUST NOT be written"
    );
    assert_eq!(
        env.get("OPENAI_API_KEY").map(String::as_str),
        Some("preserve-me"),
        "strip must not execute on bearer NUL"
    );
}

#[test]
fn codex_seed_refuses_to_overwrite_unparseable_toml_when_overwrite_false() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let target = home.join(".codex/config.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let garbage: &[u8] = b"\xff\xff this is not toml \xee\n";
    fs::write(&target, garbage).unwrap();
    let original = fs::read(&target).unwrap();

    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-test");
    let mut env: HashMap<String, String> = HashMap::new();

    let err =
        apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
            .unwrap_err();

    assert!(
        err.to_string().contains("refusing to overwrite")
            || err.to_string().contains("failed to parse"),
        "expected refuse-to-overwrite error: {err:#}"
    );
    let after = fs::read(&target).unwrap();
    assert_eq!(after, original, "original bytes must be unchanged");
}

#[cfg(unix)]
#[test]
fn codex_seed_does_not_follow_preexisting_temp_symlink() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    let target = home.join(".codex/config.toml");
    let old_temp_path = home.join(".codex/config.toml.hm-tmp");
    let outside = tmp.path().join("outside.toml");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&outside, "outside").unwrap();
    std::os::unix::fs::symlink(&outside, &old_temp_path).unwrap();

    let resolved = proxy_profile_with_gateway(vec!["openai"], "bearer-test");
    let mut env: HashMap<String, String> = HashMap::new();
    apply_codex_config_seed_strategy(&codex_config_seed_injection(), &resolved, &mut env, home)
        .unwrap();

    assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
    assert!(
        !fs::symlink_metadata(&target)
            .unwrap()
            .file_type()
            .is_symlink(),
        "config.toml must be a real file"
    );
}
