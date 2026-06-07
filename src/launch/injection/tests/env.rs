use super::*;
use crate::launch::injection::apply_env_strategy;

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
fn env_strategy_rejects_gateway_without_bearer() {
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("ANTHROPIC_API_KEY".to_string(), "host-key".to_string());
    env.insert("ANTHROPIC_BASE_URL".to_string(), "host-url".to_string());
    let mut resolved = empty_profile("no-bearer");
    resolved.gateway = Some(ResolvedGateway {
        base_url: "https://gw.example/v1".to_string(),
        bearer: None,
        providers: vec!["anthropic".to_string()],
        endpoint_strip_v1_override: None,
        provider_headers: HashMap::new(),
    });

    let err = apply_env_strategy(&claude_env_injection(), &resolved, &mut env).unwrap_err();

    assert!(
        err.to_string().contains("gateway.bearer"),
        "expected gateway.bearer required error: {err:#}"
    );
    assert_eq!(
        env.get("ANTHROPIC_API_KEY").map(String::as_str),
        Some("host-key"),
        "missing bearer must not strip host api_key_env (fail-closed)"
    );
    assert_eq!(
        env.get("ANTHROPIC_BASE_URL").map(String::as_str),
        Some("host-url"),
        "missing bearer must not strip host endpoint env (fail-closed)"
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
fn env_strategy_rejects_empty_gateway_bearer_before_any_env_mutation() {
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("ANTHROPIC_API_KEY".to_string(), "host-key".to_string());
    env.insert("ANTHROPIC_BASE_URL".to_string(), "host-url".to_string());
    let resolved = proxy_profile_with_gateway(vec!["anthropic"], "");

    let err = apply_env_strategy(&claude_env_injection(), &resolved, &mut env).unwrap_err();

    assert!(
        err.to_string().contains("bearer is empty or whitespace"),
        "expected empty-bearer rejection, got: {err:#}"
    );
    assert_eq!(
        env.get("ANTHROPIC_API_KEY").map(String::as_str),
        Some("host-key"),
        "empty bearer must not strip host api_key_env"
    );
    assert_eq!(
        env.get("ANTHROPIC_BASE_URL").map(String::as_str),
        Some("host-url"),
        "empty bearer must not strip host endpoint env"
    );
}

#[test]
fn env_strategy_rejects_whitespace_only_gateway_bearer() {
    let mut env: HashMap<String, String> = HashMap::new();
    let resolved = proxy_profile_with_gateway(vec!["anthropic"], "   \t  ");

    let err = apply_env_strategy(&claude_env_injection(), &resolved, &mut env).unwrap_err();

    assert!(
        err.to_string().contains("bearer is empty or whitespace"),
        "expected whitespace-bearer rejection, got: {err:#}"
    );
    assert!(
        !env.contains_key("ANTHROPIC_API_KEY"),
        "whitespace bearer must not be inserted"
    );
}

#[test]
fn env_strategy_rejects_empty_legacy_bearer() {
    let mut env: HashMap<String, String> = HashMap::new();
    let mut p = empty_profile("legacy");
    p.endpoint = Some("https://llm.example/v1".to_string());
    p.bearer = Some("".to_string());

    let err = apply_env_strategy(&claude_env_injection(), &p, &mut env).unwrap_err();

    assert!(
        err.to_string().contains("bearer is empty or whitespace"),
        "expected empty-bearer rejection on legacy path, got: {err:#}"
    );
    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
    assert!(!env.contains_key("ANTHROPIC_BASE_URL"));
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
