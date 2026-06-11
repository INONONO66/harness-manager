use std::path::Path;

pub(super) use crate::runtimes::types::AuthStatus;

pub(super) fn probe_data_dir_json(
    data_subdir: &str,
    file_name: &str,
    label: &str,
) -> Option<AuthStatus> {
    let file = resolve_data_file(data_subdir, file_name)?;
    probe_data_dir_json_at(&file, label)
}

fn probe_data_dir_json_at(file: &Path, label: &str) -> Option<AuthStatus> {
    count_valid_credentials_in_file(file, label, is_valid_opencode_credential)
}

pub(super) fn probe_provider_auth_file(
    config_dir: Option<&Path>,
    relative_path: &str,
    label: &str,
) -> Option<AuthStatus> {
    let dir = config_dir?;
    let file = dir.join(relative_path);
    count_valid_credentials_in_file(&file, label, is_valid_pi_credential)
}

fn count_valid_credentials_in_file<F>(file: &Path, label: &str, is_valid: F) -> Option<AuthStatus>
where
    F: Fn(&serde_json::Value) -> bool,
{
    if !file.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let object = json.as_object()?;
    let valid_count = object.values().filter(|v| is_valid(v)).count();
    if valid_count == 0 {
        return None;
    }
    Some(AuthStatus::Valid {
        detail: format!("{} ({} providers)", label, valid_count),
    })
}

/// OpenCode `auth.json`: Schema.Union([Oauth, Api, WellKnown]).
/// Source: openai-equivalent at sst/opencode packages/opencode/src/auth/index.ts.
fn is_valid_opencode_credential(value: &serde_json::Value) -> bool {
    is_valid_typed_credential(value, &["api", "oauth", "wellknown"])
}

/// Pi `auth.json`: API key + OAuth records.
/// Source: earendil-works/pi packages/coding-agent/docs/providers.md +
/// custom-provider.md (OAuthCredentials interface).
pub(super) fn is_valid_pi_credential(value: &serde_json::Value) -> bool {
    is_valid_typed_credential(value, &["api_key", "oauth"])
}

fn is_valid_typed_credential(value: &serde_json::Value, allowed_types: &[&str]) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    let Some(type_str) = obj.get("type").and_then(|v| v.as_str()) else {
        return false;
    };
    if !allowed_types.contains(&type_str) {
        return false;
    }
    let non_empty = |key: &str| -> bool {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    };
    match type_str {
        "api" | "api_key" => non_empty("key"),
        "oauth" => {
            non_empty("access") && non_empty("refresh") && is_valid_expires(obj.get("expires"))
        }
        "wellknown" => non_empty("key") && non_empty("token"),
        _ => false,
    }
}

/// OAuth `expires` per upstream is a non-negative INTEGER literal (NonNegativeInt
/// in OpenCode; milliseconds-since-epoch integer in Pi). Accept ONLY JSON
/// non-negative integers (`Value::as_u64`). Reject negative values, fractional
/// floats (`1.5`), integer-valued floats (`1.0`, `1e3`), and large fractional
/// values that f64 conversion would silently round to an integer
/// (`9007199254740992.5`) — neither runtime writes these. Strictly integer-only
/// validation prevents the f64-precision attack surface entirely.
fn is_valid_expires(value: Option<&serde_json::Value>) -> bool {
    value.and_then(serde_json::Value::as_u64).is_some()
}

fn resolve_data_file(data_subdir: &str, file_name: &str) -> Option<std::path::PathBuf> {
    if let Some(f) = dirs::data_dir()
        .map(|d| d.join(data_subdir).join(file_name))
        .filter(|f| f.is_file())
    {
        return Some(f);
    }
    dirs::home_dir()
        .map(|h| h.join(".local/share").join(data_subdir).join(file_name))
        .filter(|f| f.is_file())
}

#[cfg(test)]
mod data_dir_json_tests {
    use super::{probe_data_dir_json_at, AuthStatus};
    use std::path::PathBuf;

    fn write_provider_file(label: &str, body: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "hm-data-dir-json-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("auth.json");
        std::fs::write(&file, body).unwrap();
        file
    }

    #[test]
    fn data_dir_all_null_providers_returns_none() {
        let file = write_provider_file(
            "all-null",
            r#"{"openai": null, "anthropic": null, "google": null}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "all-null provider entries must not report Valid; got {result:?}"
        );
    }

    #[test]
    fn data_dir_one_valid_api_credential_among_nulls() {
        let file = write_provider_file(
            "mixed",
            r#"{"openai": null, "anthropic": {"type": "api", "key": "sk-real"}, "google": null}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("(1 providers)"),
                    "expected '(1 providers)' counting only valid credential entries, got: {detail}"
                );
            }
            other => panic!("expected Valid with count, got {other:?}"),
        }
    }

    #[test]
    fn data_dir_multiple_valid_credentials_counted() {
        let file = write_provider_file(
            "two-real",
            r#"{"openai": {"type": "api", "key": "sk-1"}, "anthropic": {"type": "api", "key": "sk-2"}, "stale": null}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("(2 providers)"),
                    "expected '(2 providers)' from 2 valid credentials + 1 null, got: {detail}"
                );
            }
            other => panic!("expected Valid with count, got {other:?}"),
        }
    }

    #[test]
    fn data_dir_rejects_pi_style_api_key_type() {
        let file = write_provider_file(
            "pi-style-rejected",
            r#"{"anthropic": {"type": "api_key", "key": "sk-pi-style"}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "OpenCode probe must reject Pi-style 'api_key' (upstream OpenCode only knows 'api'); got {result:?}"
        );
    }

    #[test]
    fn data_dir_oauth_with_negative_expires_returns_none() {
        let file = write_provider_file(
            "oauth-neg-expires",
            r#"{"x": {"type": "oauth", "access": "a", "refresh": "r", "expires": -1}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "negative expires must reject oauth (NonNegativeInt upstream); got {result:?}"
        );
    }

    #[test]
    fn data_dir_oauth_with_fractional_expires_returns_none() {
        let file = write_provider_file(
            "oauth-frac-expires",
            r#"{"x": {"type": "oauth", "access": "a", "refresh": "r", "expires": 1.5}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "fractional expires must reject oauth (integer milliseconds upstream); got {result:?}"
        );
    }

    #[test]
    fn data_dir_oauth_with_integer_valued_float_expires_returns_none() {
        let file = write_provider_file(
            "oauth-float-int-expires",
            r#"{"x": {"type": "oauth", "access": "a", "refresh": "r", "expires": 1.0}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "integer-valued float expires (1.0) is still f64-backed and must be rejected — upstream writes integer literals; got {result:?}"
        );
    }

    #[test]
    fn data_dir_oauth_with_large_fractional_expires_returns_none() {
        let file = write_provider_file(
            "oauth-huge-frac-expires",
            r#"{"x": {"type": "oauth", "access": "a", "refresh": "r", "expires": 9007199254740992.5}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "f64 precision can round 9007199254740992.5 to an integer; this must still be rejected because as_u64() never succeeds for f64-backed values; got {result:?}"
        );
    }

    #[test]
    fn data_dir_oauth_with_exponent_expires_returns_none() {
        let file = write_provider_file(
            "oauth-exp-expires",
            r#"{"x": {"type": "oauth", "access": "a", "refresh": "r", "expires": 1e3}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "exponent literal 1e3 is f64-backed; must be rejected; got {result:?}"
        );
    }

    #[test]
    fn data_dir_oauth_with_zero_expires_returns_valid() {
        let file = write_provider_file(
            "oauth-zero-expires",
            r#"{"x": {"type": "oauth", "access": "a", "refresh": "r", "expires": 0}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            matches!(result, Some(AuthStatus::Valid { .. })),
            "zero expires is non-negative integer; allowed (stale-but-typed token, refresh path will handle it)"
        );
    }

    #[test]
    fn data_dir_oauth_credential_with_all_fields_counts() {
        let file = write_provider_file(
            "oauth",
            r#"{"anthropic": {"type": "oauth", "access": "a", "refresh": "r", "expires": 12345}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(matches!(result, Some(AuthStatus::Valid { .. })));
    }

    #[test]
    fn data_dir_wellknown_credential_counts() {
        let file = write_provider_file(
            "wellknown",
            r#"{"copilot": {"type": "wellknown", "key": "k", "token": "t"}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(matches!(result, Some(AuthStatus::Valid { .. })));
    }

    #[test]
    fn data_dir_credential_without_type_field_returns_none() {
        let file = write_provider_file(
            "no-type",
            r#"{"openai": {"key": "k1"}, "anthropic": {"key": "k2"}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "untyped credential records (no 'type' field) must NOT count as valid; got {result:?}"
        );
    }

    #[test]
    fn data_dir_api_credential_with_empty_key_returns_none() {
        let file =
            write_provider_file("api-empty-key", r#"{"openai": {"type": "api", "key": ""}}"#);
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(result.is_none(), "got {result:?}");
    }

    #[test]
    fn data_dir_api_credential_missing_key_returns_none() {
        let file = write_provider_file("api-no-key", r#"{"openai": {"type": "api"}}"#);
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "type='api' missing 'key' must not count: got {result:?}"
        );
    }

    #[test]
    fn data_dir_oauth_credential_missing_refresh_returns_none() {
        let file = write_provider_file(
            "oauth-no-refresh",
            r#"{"anthropic": {"type": "oauth", "access": "a", "expires": 1}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn data_dir_oauth_credential_missing_expires_returns_none() {
        let file = write_provider_file(
            "oauth-no-expires",
            r#"{"anthropic": {"type": "oauth", "access": "a", "refresh": "r"}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn data_dir_oauth_credential_non_numeric_expires_returns_none() {
        let file = write_provider_file(
            "oauth-string-expires",
            r#"{"anthropic": {"type": "oauth", "access": "a", "refresh": "r", "expires": "soon"}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(
            result.is_none(),
            "non-numeric expires must reject oauth credential: got {result:?}"
        );
    }

    #[test]
    fn data_dir_unknown_credential_type_returns_none() {
        let file = write_provider_file(
            "unknown-type",
            r#"{"openai": {"type": "magic-future-mode", "key": "k"}}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn data_dir_empty_object_returns_none() {
        let file = write_provider_file("empty", "{}");
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn data_dir_non_object_returns_none() {
        let file = write_provider_file("array", "[1, 2, 3]");
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        assert!(result.is_none());
    }
}

#[cfg(test)]
mod provider_auth_file_tests {
    use super::{probe_provider_auth_file, AuthStatus};
    use std::path::PathBuf;

    fn write_pi_auth(label: &str, body: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "hm-pi-auth-test-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("auth.json"), body).unwrap();
        dir
    }

    #[test]
    fn pi_api_key_credential_returns_valid() {
        let dir = write_pi_auth(
            "api-key",
            r#"{"anthropic": {"type": "api_key", "key": "sk-ant-real"}}"#,
        );
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => assert!(detail.contains("(1 providers)")),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn pi_oauth_credential_returns_valid() {
        let dir = write_pi_auth(
            "oauth",
            r#"{"google": {"type": "oauth", "access": "a", "refresh": "r", "expires": 12345}}"#,
        );
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(result, Some(AuthStatus::Valid { .. })));
    }

    #[test]
    fn pi_arbitrary_non_empty_json_returns_none() {
        let dir = write_pi_auth("garbage", r#"{"foo": "bar", "baz": 42}"#);
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "arbitrary key/value pairs without credential-shape values must NOT report Valid; got {result:?}"
        );
    }

    #[test]
    fn pi_incomplete_oauth_credential_returns_none() {
        let dir = write_pi_auth("incomplete-oauth", r#"{"anthropic": {"type": "oauth"}}"#);
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn pi_empty_object_returns_none() {
        let dir = write_pi_auth("empty", "{}");
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn pi_missing_file_returns_none() {
        let dir = std::env::temp_dir().join(format!("hm-pi-auth-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn pi_no_config_dir_returns_none() {
        let result = probe_provider_auth_file(None, "auth.json", "Provider auth");
        assert!(result.is_none());
    }

    #[test]
    fn pi_mixed_valid_and_garbage_counts_only_valid() {
        let dir = write_pi_auth(
            "mixed",
            r#"{"good1": {"type": "api_key", "key": "sk-1"}, "bad": {"foo": "bar"}, "good2": {"type": "api_key", "key": "sk-2"}}"#,
        );
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => assert!(
                detail.contains("(2 providers)"),
                "expected (2 providers) counting only credential-shape entries, got {detail}"
            ),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn pi_rejects_opencode_style_api_type() {
        let dir = write_pi_auth(
            "opencode-style-rejected",
            r#"{"anthropic": {"type": "api", "key": "sk-opencode-style"}}"#,
        );
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "Pi probe must reject OpenCode-style 'api' (Pi uses 'api_key'); got {result:?}"
        );
    }

    #[test]
    fn pi_rejects_opencode_style_wellknown_type() {
        let dir = write_pi_auth(
            "opencode-wellknown-rejected",
            r#"{"copilot": {"type": "wellknown", "key": "k", "token": "t"}}"#,
        );
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "Pi probe must reject OpenCode-specific 'wellknown' (not supported by Pi); got {result:?}"
        );
    }

    #[test]
    fn pi_oauth_with_negative_expires_returns_none() {
        let dir = write_pi_auth(
            "oauth-neg-expires",
            r#"{"google": {"type": "oauth", "access": "a", "refresh": "r", "expires": -1}}"#,
        );
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn pi_oauth_with_fractional_expires_returns_none() {
        let dir = write_pi_auth(
            "oauth-frac-expires",
            r#"{"google": {"type": "oauth", "access": "a", "refresh": "r", "expires": 12.5}}"#,
        );
        let result = probe_provider_auth_file(Some(&dir), "auth.json", "Provider auth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }
}
