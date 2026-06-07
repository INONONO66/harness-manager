use super::*;
use crate::launch::injection::{
    apply_provider_config_seed_strategy, validate_provider_config_seed,
};
use std::collections::HashMap;

#[test]
fn provider_config_seed_rejects_empty_gateway_bearer() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = proxy_profile_with_gateway(vec!["openai"], "");
    let mut env = HashMap::new();

    let err = apply_provider_config_seed_strategy(
        &opencode_seed_injection(),
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
        !tmp.path().join(".config/opencode/opencode.json").exists(),
        "seed file must not be written when bearer is empty"
    );
}

#[test]
fn provider_config_seed_rejects_whitespace_legacy_bearer() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = empty_profile("legacy");
    p.endpoint = Some("https://legacy.example/v1".to_string());
    p.bearer = Some("   \n  ".to_string());
    let mut env = HashMap::new();

    let err =
        apply_provider_config_seed_strategy(&opencode_seed_injection(), &p, &mut env, tmp.path())
            .unwrap_err();

    assert!(
        err.to_string().contains("bearer is empty or whitespace"),
        "expected whitespace bearer rejection, got: {err:#}"
    );
}

#[test]
fn provider_config_seed_rejects_bearer_crlf_even_without_header_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = proxy_profile_with_gateway(vec!["openai"], "good\r\nX-Injected: evil");
    let mut env = HashMap::new();

    let err = apply_provider_config_seed_strategy(
        &opencode_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
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
    let mut env = HashMap::new();

    let err = apply_provider_config_seed_strategy(
        &opencode_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
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
    let mut env = HashMap::new();

    let err = apply_provider_config_seed_strategy(
        &opencode_seed_injection(),
        &resolved,
        &mut env,
        tmp.path(),
    )
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
