use std::path::Path;

use super::jwt::token_to_auth_status;
pub(super) use crate::runtimes::types::AuthStatus;

fn non_empty_json_str(value: Option<&serde_json::Value>) -> Option<&str> {
    let s = value.and_then(|v| v.as_str())?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn has_real_oauth_tokens(json: &serde_json::Value) -> Option<&str> {
    let tokens = json.get("tokens").filter(|v| v.is_object())?;
    let access = non_empty_json_str(tokens.get("access_token"))?;
    let _refresh = non_empty_json_str(tokens.get("refresh_token"))?;
    Some(access)
}

pub(super) fn probe_codex_auth_file(
    config_dir: Option<&Path>,
    relative_path: &str,
    oauth_label: &str,
    api_key_label: &str,
    pat_label: Option<&str>,
    agent_identity_label: Option<&str>,
) -> Option<AuthStatus> {
    let dir = config_dir?;
    let file = dir.join(relative_path);
    if !file.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(&file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    if let Some(mode) = non_empty_json_str(json.get("auth_mode")) {
        return probe_codex_explicit_mode(
            mode,
            &json,
            oauth_label,
            api_key_label,
            pat_label,
            agent_identity_label,
        );
    }

    if let Some(label) = pat_label {
        if non_empty_json_str(json.get("personal_access_token")).is_some() {
            return Some(AuthStatus::Valid {
                detail: label.to_string(),
            });
        }
    }

    if non_empty_json_str(json.get("OPENAI_API_KEY")).is_some() {
        return Some(AuthStatus::Valid {
            detail: api_key_label.to_string(),
        });
    }

    if let Some(access) = has_real_oauth_tokens(&json) {
        return Some(token_to_auth_status(access, oauth_label));
    }

    None
}

fn probe_codex_explicit_mode(
    mode: &str,
    json: &serde_json::Value,
    oauth_label: &str,
    api_key_label: &str,
    pat_label: Option<&str>,
    agent_identity_label: Option<&str>,
) -> Option<AuthStatus> {
    let normalized = mode.to_ascii_lowercase();
    match normalized.as_str() {
        "apikey" | "api_key" => {
            non_empty_json_str(json.get("OPENAI_API_KEY")).map(|_| AuthStatus::Valid {
                detail: api_key_label.to_string(),
            })
        }
        "chatgpt" | "chatgptauthtokens" => {
            has_real_oauth_tokens(json).map(|access| token_to_auth_status(access, oauth_label))
        }
        "personalaccesstoken" | "personal_access_token" => {
            let label = pat_label?;
            non_empty_json_str(json.get("personal_access_token")).map(|_| AuthStatus::Valid {
                detail: label.to_string(),
            })
        }
        "agentidentity" | "agent_identity" => {
            let label = agent_identity_label?;
            non_empty_json_str(json.get("agent_identity"))
                .map(|token| token_to_auth_status(token, label))
        }
        _ => None,
    }
}

#[cfg(test)]
mod codex_auth_file_tests {
    use super::{probe_codex_auth_file, AuthStatus};
    use std::path::PathBuf;

    fn write_auth(label: &str, body: &str) -> (PathBuf, String) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "hm-codex-auth-test-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("auth.json"), body).unwrap();
        (dir, "auth.json".to_string())
    }

    fn probe(dir: &std::path::Path, file: &str) -> Option<AuthStatus> {
        probe_codex_auth_file(
            Some(dir),
            file,
            "ChatGPT OAuth",
            "API key (OPENAI_API_KEY)",
            Some("Personal access token"),
            Some("Agent identity"),
        )
    }

    #[test]
    fn tokens_null_returns_none() {
        let (dir, f) = write_auth("null", r#"{"tokens": null}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none(), "got {result:?}");
    }

    #[test]
    fn tokens_empty_object_returns_none() {
        let (dir, f) = write_auth("empty-obj", r#"{"tokens": {}}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none(), "got {result:?}");
    }

    #[test]
    fn tokens_as_string_returns_none() {
        let (dir, f) = write_auth("string", r#"{"tokens": "not-an-object"}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "tokens must be object, not string: got {result:?}"
        );
    }

    #[test]
    fn tokens_as_array_returns_none() {
        let (dir, f) = write_auth("array", r#"{"tokens": ["not-an-object"]}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "tokens must be object, not array: got {result:?}"
        );
    }

    #[test]
    fn tokens_metadata_only_returns_none() {
        let (dir, f) = write_auth("metadata-only", r#"{"tokens": {"account_id": "acct-123"}}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "metadata without access_token/refresh_token must NOT be OAuth: got {result:?}"
        );
    }

    #[test]
    fn tokens_access_only_returns_none() {
        let (dir, f) = write_auth("access-only", r#"{"tokens": {"access_token": "a"}}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "access_token without refresh_token must NOT be OAuth: got {result:?}"
        );
    }

    #[test]
    fn tokens_refresh_only_returns_none() {
        let (dir, f) = write_auth("refresh-only", r#"{"tokens": {"refresh_token": "r"}}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "refresh_token without access_token must NOT be OAuth: got {result:?}"
        );
    }

    #[test]
    fn tokens_both_present_returns_oauth_valid() {
        let (dir, f) = write_auth(
            "both-real",
            r#"{"tokens": {"access_token": "a-real-token", "refresh_token": "r-real-token"}}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("ChatGPT OAuth"),
                    "OAuth label expected: {detail}"
                );
            }
            other => panic!("expected Valid OAuth, got {other:?}"),
        }
    }

    #[test]
    fn tokens_both_whitespace_returns_none() {
        let (dir, f) = write_auth(
            "both-ws",
            r#"{"tokens": {"access_token": "   ", "refresh_token": "   "}}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "whitespace tokens must NOT be OAuth: got {result:?}"
        );
    }

    #[test]
    fn openai_api_key_only_returns_api_key_valid() {
        let (dir, f) = write_auth("api-only", r#"{"OPENAI_API_KEY": "sk-test-1234"}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("OPENAI_API_KEY"),
                    "API key label expected: {detail}"
                );
            }
            other => panic!("expected Valid API key, got {other:?}"),
        }
    }

    #[test]
    fn openai_api_key_with_null_tokens_returns_api_key_valid() {
        let (dir, f) = write_auth(
            "api-with-null-tokens",
            r#"{"OPENAI_API_KEY":"sk-test-key","tokens":null,"last_refresh":null}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("OPENAI_API_KEY"),
                    "API key label expected: {detail}"
                );
            }
            other => panic!("expected Valid API key, got {other:?}"),
        }
    }

    #[test]
    fn empty_openai_api_key_returns_none() {
        let (dir, f) = write_auth("api-empty", r#"{"OPENAI_API_KEY": ""}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none(), "got {result:?}");
    }

    #[test]
    fn personal_access_token_only_returns_pat_valid() {
        let (dir, f) = write_auth("pat", r#"{"personal_access_token": "pat-xyz"}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("Personal access token"),
                    "PAT label expected: {detail}"
                );
            }
            other => panic!("expected Valid PAT, got {other:?}"),
        }
    }

    #[test]
    fn agent_identity_only_without_auth_mode_returns_none() {
        let (dir, f) = write_auth("ai-no-mode", r#"{"agent_identity": "agent-jwt-token"}"#);
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "agent_identity alone (without explicit auth_mode) must NOT be reported as Valid; upstream Codex requires auth_mode = agentIdentity to activate. Got: {result:?}"
        );
    }

    #[test]
    fn empty_object_returns_none() {
        let (dir, f) = write_auth("empty", "{}");
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none(), "got {result:?}");
    }

    #[test]
    fn priority_api_key_wins_over_oauth_tokens_when_no_auth_mode() {
        let (dir, f) = write_auth(
            "no-mode-api-and-tokens",
            r#"{"OPENAI_API_KEY": "sk-test", "tokens": {"access_token": "a", "refresh_token": "r"}}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("OPENAI_API_KEY"),
                    "no auth_mode + API key + tokens: API key wins per upstream AuthDotJson::resolved_mode fallback. Got: {detail}"
                );
            }
            other => panic!("expected API key Valid (no auth_mode fallback), got {other:?}"),
        }
    }

    #[test]
    fn priority_pat_wins_over_api_key_when_no_auth_mode() {
        let (dir, f) = write_auth(
            "no-mode-pat-and-api",
            r#"{"personal_access_token": "pat-1", "OPENAI_API_KEY": "sk-test"}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("Personal access token"),
                    "no auth_mode + PAT + API key: PAT wins (more specific). Got: {detail}"
                );
            }
            other => panic!("expected PAT Valid (no auth_mode fallback), got {other:?}"),
        }
    }

    #[test]
    fn no_auth_mode_with_only_oauth_tokens_reports_oauth() {
        let (dir, f) = write_auth(
            "tokens-only-no-mode",
            r#"{"tokens": {"access_token": "a-real", "refresh_token": "r-real"}}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("ChatGPT OAuth"),
                    "with no auth_mode and no PAT/API key, OAuth tokens are the last fallback. Got: {detail}"
                );
            }
            other => panic!("expected OAuth (final fallback), got {other:?}"),
        }
    }

    #[test]
    fn explicit_auth_mode_apikey_with_key_returns_api_key_valid() {
        let (dir, f) = write_auth(
            "mode-apikey",
            r#"{"auth_mode": "apikey", "OPENAI_API_KEY": "sk-real"}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(result, Some(AuthStatus::Valid { .. })));
    }

    #[test]
    fn explicit_auth_mode_apikey_without_key_returns_none() {
        let (dir, f) = write_auth(
            "mode-apikey-no-key",
            r#"{"auth_mode": "apikey", "tokens": {"access_token": "stale", "refresh_token": "stale"}}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "explicit auth_mode = apikey with missing OPENAI_API_KEY must NOT fall through to stale OAuth tokens: got {result:?}"
        );
    }

    #[test]
    fn explicit_auth_mode_chatgpt_with_full_tokens_returns_oauth() {
        let (dir, f) = write_auth(
            "mode-chatgpt",
            r#"{"auth_mode": "chatgpt", "tokens": {"access_token": "a", "refresh_token": "r"}}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(result, Some(AuthStatus::Valid { .. })));
    }

    #[test]
    fn explicit_auth_mode_chatgpt_without_tokens_returns_none() {
        let (dir, f) = write_auth(
            "mode-chatgpt-no-tokens",
            r#"{"auth_mode": "chatgpt", "OPENAI_API_KEY": "stale-key"}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "explicit auth_mode = chatgpt with missing tokens must NOT fall through to stale API key: got {result:?}"
        );
    }

    #[test]
    fn explicit_auth_mode_personal_access_token_with_value_returns_pat() {
        let (dir, f) = write_auth(
            "mode-pat",
            r#"{"auth_mode": "personalAccessToken", "personal_access_token": "pat-real"}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(detail.contains("Personal access token"));
            }
            other => panic!("expected Valid PAT, got {other:?}"),
        }
    }

    #[test]
    fn explicit_auth_mode_agent_identity_with_value_returns_agent_identity() {
        let (dir, f) = write_auth(
            "mode-ai",
            r#"{"auth_mode": "agentIdentity", "agent_identity": "agent-jwt"}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(detail.contains("Agent identity"));
            }
            other => panic!("expected Valid agent identity, got {other:?}"),
        }
    }

    #[test]
    fn explicit_auth_mode_agent_identity_without_value_returns_none() {
        let (dir, f) = write_auth(
            "mode-ai-no-value",
            r#"{"auth_mode": "agentIdentity", "OPENAI_API_KEY": "stale"}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "explicit auth_mode = agentIdentity with no agent_identity value must NOT fall back: got {result:?}"
        );
    }

    #[test]
    fn explicit_auth_mode_unknown_value_returns_none() {
        let (dir, f) = write_auth(
            "mode-unknown",
            r#"{"auth_mode": "futureMode", "OPENAI_API_KEY": "sk-real"}"#,
        );
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "unknown auth_mode must not silently fall through to a known field: got {result:?}"
        );
    }

    #[test]
    fn missing_file_returns_none() {
        let dir =
            std::env::temp_dir().join(format!("hm-codex-auth-test-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = probe(&dir, "auth.json");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn malformed_json_returns_none() {
        let (dir, f) = write_auth("bad-json", "not json at all");
        let result = probe(&dir, &f);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn optional_labels_skipped_when_none() {
        let (dir, f) = write_auth("pat-no-label", r#"{"personal_access_token": "pat-xyz"}"#);
        let result = probe_codex_auth_file(
            Some(dir.as_path()),
            &f,
            "ChatGPT OAuth",
            "API key",
            None,
            None,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "PAT must not produce Valid when no pat_label declared in manifest: got {result:?}"
        );
    }
}
