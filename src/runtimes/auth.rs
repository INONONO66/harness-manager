use std::path::Path;

use super::manifest::AuthProbeRecord;
use super::types::AuthStatus;

/// Run all auth probes and collect every match (not first-match-wins).
pub fn probe_auth_all(probes: &[AuthProbeRecord], config_dir: Option<&Path>) -> Vec<AuthStatus> {
    let mut results = Vec::new();
    for probe in probes {
        let result = run_probe(probe, config_dir);
        if !matches!(result, AuthStatus::NotConfigured) {
            results.push(result);
        }
    }
    results
}

fn run_probe(probe: &AuthProbeRecord, config_dir: Option<&Path>) -> AuthStatus {
    match probe {
        AuthProbeRecord::EnvKeys { vars, label } => probe_env_keys(vars, label),
        AuthProbeRecord::JsonFile {
            relative_path,
            existence_field,
            label,
        } => probe_json_file(config_dir, relative_path, existence_field, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::OAuthFile {
            relative_path,
            token_field,
            label,
        } => probe_oauth_file(config_dir, relative_path, token_field, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::NestedOAuthFile {
            relative_path,
            path,
            label,
        } => probe_nested_oauth(config_dir, relative_path, path, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::DataDirJsonFile {
            data_subdir,
            file_name,
            label,
        } => {
            probe_data_dir_json(data_subdir, file_name, label).unwrap_or(AuthStatus::NotConfigured)
        }
        AuthProbeRecord::ProviderAuthFile {
            relative_path,
            label,
        } => probe_provider_auth_file(config_dir, relative_path, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::KeychainHeuristic {
            marker_file,
            keychain_service,
            label,
        } => probe_keychain(config_dir, marker_file, keychain_service.as_deref(), label),
        AuthProbeRecord::CodexAuthFile {
            relative_path,
            oauth_label,
            api_key_label,
            personal_access_token_label,
            agent_identity_label,
        } => probe_codex_auth_file(
            config_dir,
            relative_path,
            oauth_label,
            api_key_label,
            personal_access_token_label.as_deref(),
            agent_identity_label.as_deref(),
        )
        .unwrap_or(AuthStatus::NotConfigured),
    }
}

fn probe_env_keys(vars: &[String], label: &str) -> AuthStatus {
    for var in vars {
        if std::env::var(var).is_ok_and(|v| !v.trim().is_empty()) {
            return AuthStatus::Valid {
                detail: format!("{} ({})", label, var),
            };
        }
    }
    AuthStatus::NotConfigured
}

fn probe_json_file(
    config_dir: Option<&Path>,
    relative_path: &str,
    existence_field: &str,
    label: &str,
) -> Option<AuthStatus> {
    let dir = config_dir?;
    let file = dir.join(relative_path);
    if !file.is_file() {
        return None;
    }

    let content = std::fs::read_to_string(&file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    if json.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return None;
    }

    if !existence_field.is_empty() {
        let field = json
            .get(existence_field)
            .or_else(|| json.get(to_camel(existence_field)))?;
        if !is_meaningful_json_value(field) {
            return None;
        }
        return Some(AuthStatus::Valid {
            detail: label.to_string(),
        });
    }

    let valid_credentials = json
        .as_object()
        .map(|obj| obj.values().any(is_valid_pi_credential))
        .unwrap_or(false);
    if !valid_credentials {
        return None;
    }
    Some(AuthStatus::Valid {
        detail: label.to_string(),
    })
}

fn is_meaningful_json_value(v: &serde_json::Value) -> bool {
    use serde_json::Value;
    match v {
        Value::Null | Value::Number(_) | Value::Bool(_) => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Object(o) => o.values().any(is_meaningful_json_value),
        Value::Array(a) => a.iter().any(is_meaningful_json_value),
    }
}

fn probe_oauth_file(
    config_dir: Option<&Path>,
    relative_path: &str,
    token_field: &str,
    label: &str,
) -> Option<AuthStatus> {
    let dir = config_dir?;
    let file = dir.join(relative_path);
    if !file.is_file() {
        return None;
    }

    let content = std::fs::read_to_string(&file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let token = json
        .get(token_field)
        .or_else(|| json.get(to_camel(token_field)))
        .and_then(|v| v.as_str())
        .filter(|t| !t.trim().is_empty())?;

    Some(token_to_auth_status(token, label))
}

fn probe_nested_oauth(
    config_dir: Option<&Path>,
    relative_path: &str,
    path: &[String],
    label: &str,
) -> Option<AuthStatus> {
    let dir = config_dir?;
    let file = dir.join(relative_path);
    if !file.is_file() {
        return None;
    }

    let content = std::fs::read_to_string(&file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let mut current = &json;
    for segment in path {
        current = current
            .get(segment.as_str())
            .or_else(|| current.get(to_camel(segment)))?;
    }

    let token = current.as_str().filter(|t| !t.trim().is_empty())?;
    Some(token_to_auth_status(token, label))
}

fn probe_data_dir_json(data_subdir: &str, file_name: &str, label: &str) -> Option<AuthStatus> {
    let file = resolve_data_file(data_subdir, file_name)?;
    probe_data_dir_json_at(&file, label)
}

fn probe_data_dir_json_at(file: &Path, label: &str) -> Option<AuthStatus> {
    count_valid_credentials_in_file(file, label, is_valid_opencode_credential)
}

fn probe_provider_auth_file(
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
fn is_valid_pi_credential(value: &serde_json::Value) -> bool {
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

/// OAuth `expires` per upstream is a non-negative integer (NonNegativeInt in
/// OpenCode; milliseconds-since-epoch integer in Pi). Reject negative values
/// and any fractional/non-finite numbers — these can never be a real expiry.
fn is_valid_expires(value: Option<&serde_json::Value>) -> bool {
    let Some(v) = value else {
        return false;
    };
    if let Some(i) = v.as_i64() {
        return i >= 0;
    }
    if let Some(u) = v.as_u64() {
        let _ = u;
        return true;
    }
    if let Some(f) = v.as_f64() {
        return f.is_finite() && f >= 0.0 && f.fract() == 0.0;
    }
    false
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

fn probe_codex_auth_file(
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

fn probe_keychain(
    config_dir: Option<&Path>,
    marker_file: &str,
    keychain_service: Option<&str>,
    label: &str,
) -> AuthStatus {
    if !cfg!(target_os = "macos") {
        return AuthStatus::NotConfigured;
    }
    let Some(dir) = config_dir else {
        return AuthStatus::NotConfigured;
    };
    if !(dir.is_dir() && dir.join(marker_file).is_file()) {
        return AuthStatus::NotConfigured;
    }
    let Some(service) = keychain_service else {
        return AuthStatus::NotConfigured;
    };
    if !keychain_item_exists(service) {
        return AuthStatus::NotConfigured;
    }
    AuthStatus::Valid {
        detail: label.to_string(),
    }
}

#[cfg(target_os = "macos")]
fn keychain_item_exists(service: &str) -> bool {
    match std::process::Command::new("security")
        .args(["find-generic-password", "-s", service])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_item_exists(_service: &str) -> bool {
    false
}

fn token_to_auth_status(token: &str, label: &str) -> AuthStatus {
    let expiry = decode_jwt_expiry(token);
    match expiry {
        Some(exp) if exp == "EXPIRED" => AuthStatus::Expired {
            detail: format!("{} (expired)", label),
        },
        Some(exp) => AuthStatus::Valid {
            detail: format!("{} ({})", label, exp),
        },
        None => AuthStatus::Valid {
            detail: label.to_string(),
        },
    }
}

fn to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut cap = false;
    for c in s.chars() {
        if c == '_' {
            cap = true;
        } else if cap {
            result.extend(c.to_uppercase());
            cap = false;
        } else {
            result.push(c);
        }
    }
    result
}

fn decode_jwt_expiry(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let payload = parts[1];
    let padded = match payload.len() % 4 {
        2 => format!("{}==", payload),
        3 => format!("{}=", payload),
        _ => payload.to_string(),
    };
    let decoded_str = padded.replace('-', "+").replace('_', "/");
    let bytes = base64_decode(&decoded_str)?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;

    let exp = json.get("exp")?.as_u64()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    if exp < now {
        return Some("EXPIRED".to_string());
    }

    let remaining = exp - now;
    let days = remaining / 86400;
    let hours = (remaining % 86400) / 3600;

    if days > 0 {
        Some(format!("{}d {}h left", days, hours))
    } else if hours > 0 {
        Some(format!("{}h left", hours))
    } else {
        Some(format!("{}m left", (remaining % 3600) / 60))
    }
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &b in input.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = TABLE.iter().position(|&c| c == b)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(output)
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

#[cfg(test)]
mod json_file_tests {
    use super::{probe_json_file, AuthStatus};
    use std::path::PathBuf;

    fn write_json(label: &str, body: &str) -> (PathBuf, String) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "hm-json-probe-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file_name = "auth.json".to_string();
        std::fs::write(dir.join(&file_name), body).unwrap();
        (dir, file_name)
    }

    #[test]
    fn json_file_with_null_existence_field_returns_none() {
        let (dir, file) = write_json("null-tokens", r#"{"tokens": null}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "null tokens field must not be valid auth; got {result:?}"
        );
    }

    #[test]
    fn json_file_with_empty_object_existence_field_returns_none() {
        let (dir, file) = write_json("empty-obj-tokens", r#"{"tokens": {}}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "empty {{}} tokens field must not be valid auth (logged-out state); got {result:?}"
        );
    }

    #[test]
    fn json_file_with_empty_string_existence_field_returns_none() {
        let (dir, file) = write_json("empty-str-tokens", r#"{"tokens": ""}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "empty-string tokens field must not be valid auth; got {result:?}"
        );
    }

    #[test]
    fn json_file_with_whitespace_string_existence_field_returns_none() {
        let (dir, file) = write_json("ws-str-tokens", r#"{"tokens": "   "}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "whitespace tokens field must not be valid auth; got {result:?}"
        );
    }

    #[test]
    fn json_file_with_real_nested_token_object_returns_valid() {
        let (dir, file) = write_json(
            "real-nested",
            r#"{"tokens": {"access_token": "real-secret"}}"#,
        );
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(result, Some(AuthStatus::Valid { .. })),
            "tokens object with real content must be Valid; got {result:?}"
        );
    }

    #[test]
    fn json_file_with_real_string_token_returns_valid() {
        let (dir, file) = write_json("real-str", r#"{"tokens": "real-token-abcdef"}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(result, Some(AuthStatus::Valid { .. })),
            "real string token must be Valid; got {result:?}"
        );
    }

    #[test]
    fn json_file_with_object_of_nulls_returns_none() {
        let (dir, file) = write_json(
            "deeply-null",
            r#"{"tokens": {"access_token": null, "refresh_token": null}}"#,
        );
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "tokens object whose every field is null must not be valid auth; got {result:?}"
        );
    }

    #[test]
    fn json_file_missing_existence_field_returns_none() {
        let (dir, file) = write_json("no-tokens", r#"{"other": "data"}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn json_file_no_existence_field_with_only_null_values_returns_none() {
        let (dir, file) = write_json("no-field-null", r#"{"x": null, "y": null}"#);
        let result = probe_json_file(Some(&dir), &file, "", "Pi token");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "no-existence-field probe must skip files whose every value is null; got {result:?}"
        );
    }

    #[test]
    fn json_file_no_existence_field_legacy_pi_credential_returns_valid() {
        let (dir, file) = write_json(
            "no-field-real",
            r#"{"anthropic": {"type": "api_key", "key": "sk-real"}}"#,
        );
        let result = probe_json_file(Some(&dir), &file, "", "Pi token");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(result, Some(AuthStatus::Valid { .. })),
            "legacy Pi-style probe (json-file + empty existence_field) must accept a typed Pi credential shape, auto-upgrading legacy hm-init copies to safe validation; got {result:?}"
        );
    }

    #[test]
    fn json_file_no_existence_field_rejects_arbitrary_non_credential_json() {
        let (dir, file) = write_json("no-field-arbitrary", r#"{"foo": "bar", "baz": 42}"#);
        let result = probe_json_file(Some(&dir), &file, "", "Pi token");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "legacy probe must NOT accept arbitrary JSON (Oracle's regression target for legacy hm-init Pi users); got {result:?}"
        );
    }

    #[test]
    fn json_file_no_existence_field_rejects_untyped_record() {
        let (dir, file) = write_json("no-field-untyped", r#"{"openai": {"key": "k"}}"#);
        let result = probe_json_file(Some(&dir), &file, "", "Pi token");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "legacy probe must require typed credential record; got {result:?}"
        );
    }

    #[test]
    fn json_file_with_bool_true_existence_field_returns_none() {
        let (dir, file) = write_json("bool-true", r#"{"tokens": true}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "bool true is not a real OAuth token; got {result:?}"
        );
    }

    #[test]
    fn json_file_with_number_existence_field_returns_none() {
        let (dir, file) = write_json("number", r#"{"tokens": 0}"#);
        let result = probe_json_file(Some(&dir), &file, "tokens", "ChatGPT OAuth");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "number is not a real OAuth token; got {result:?}"
        );
    }

    #[test]
    fn json_file_no_existence_field_with_only_bools_and_numbers_returns_none() {
        let (dir, file) = write_json("scalars-only", r#"{"x": false, "y": 0}"#);
        let result = probe_json_file(Some(&dir), &file, "", "Pi token");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "no-existence-field probe must not treat bool/number placeholders as auth; got {result:?}"
        );
    }
}

#[cfg(test)]
mod keychain_probe_tests {
    use super::{probe_keychain, AuthStatus};
    use std::path::PathBuf;

    fn unique_dir(label: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let p = std::env::temp_dir().join(format!(
            "hm-keychain-test-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn unique_service(label: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!(
            "hm-test-nonexistent-keychain-{}-{}-{nanos}",
            label,
            std::process::id()
        )
    }

    #[test]
    fn keychain_probe_marker_present_but_keychain_item_missing_returns_not_configured() {
        let dir = unique_dir("marker-only");
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let service = unique_service("missing");
        let result = probe_keychain(
            Some(&dir),
            "settings.json",
            Some(&service),
            "OAuth (Keychain)",
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(result, AuthStatus::NotConfigured),
            "marker file alone must NOT report Valid when keychain item is absent; got {result:?}"
        );
    }

    #[test]
    fn keychain_probe_no_marker_returns_not_configured() {
        let dir = unique_dir("no-marker");
        let service = unique_service("no-marker");
        let result = probe_keychain(
            Some(&dir),
            "settings.json",
            Some(&service),
            "OAuth (Keychain)",
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(result, AuthStatus::NotConfigured));
    }

    #[test]
    fn keychain_probe_no_config_dir_returns_not_configured() {
        let service = unique_service("no-dir");
        let result = probe_keychain(None, "settings.json", Some(&service), "OAuth (Keychain)");
        assert!(matches!(result, AuthStatus::NotConfigured));
    }

    #[test]
    fn keychain_probe_legacy_manifest_without_service_returns_not_configured() {
        let dir = unique_dir("legacy-no-service");
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let result = probe_keychain(Some(&dir), "settings.json", None, "OAuth (Keychain)");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(result, AuthStatus::NotConfigured),
            "legacy manifest missing keychain_service must fail closed, not report Valid; got {result:?}"
        );
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn keychain_probe_on_non_macos_is_not_configured_regardless_of_marker() {
        let dir = unique_dir("non-mac");
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let result = probe_keychain(
            Some(&dir),
            "settings.json",
            Some("any-service"),
            "OAuth (Keychain)",
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(result, AuthStatus::NotConfigured));
    }
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

#[cfg(test)]
mod oauth_file_tests {
    use super::{probe_nested_oauth, probe_oauth_file, AuthStatus};
    use std::path::PathBuf;

    fn write_oauth_file(label: &str, contents: &str) -> (PathBuf, String) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!(
            "hm-oauth-test-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file_name = format!("auth-{label}.json");
        std::fs::write(dir.join(&file_name), contents).unwrap();
        (dir, file_name)
    }

    #[test]
    fn oauth_file_with_empty_token_field_returns_none() {
        let (dir, file) = write_oauth_file("empty-token", r#"{"access_token": ""}"#);

        let result = probe_oauth_file(Some(&dir), &file, "access_token", "ChatGPT OAuth");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "empty access_token must not be considered valid auth; got {result:?}"
        );
    }

    #[test]
    fn oauth_file_with_whitespace_token_field_returns_none() {
        let (dir, file) = write_oauth_file("ws-token", r#"{"access_token": "   \n  "}"#);

        let result = probe_oauth_file(Some(&dir), &file, "access_token", "ChatGPT OAuth");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "whitespace-only access_token must not be considered valid auth; got {result:?}"
        );
    }

    #[test]
    fn oauth_file_with_real_token_returns_valid() {
        let (dir, file) =
            write_oauth_file("real-token", r#"{"access_token": "real-secret-abcdef"}"#);

        let result = probe_oauth_file(Some(&dir), &file, "access_token", "ChatGPT OAuth");

        let _ = std::fs::remove_dir_all(&dir);
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("ChatGPT OAuth"),
                    "label should appear: {detail}"
                );
            }
            other => panic!("expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn nested_oauth_with_empty_leaf_token_returns_none() {
        let (dir, file) = write_oauth_file(
            "nested-empty",
            r#"{"tokens": {"oauth": {"access_token": ""}}}"#,
        );

        let path: Vec<String> = ["tokens", "oauth", "access_token"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let result = probe_nested_oauth(Some(&dir), &file, &path, "Pi OAuth");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "empty nested token must not be valid auth; got {result:?}"
        );
    }

    #[test]
    fn nested_oauth_with_whitespace_leaf_token_returns_none() {
        let (dir, file) = write_oauth_file(
            "nested-ws",
            r#"{"tokens": {"oauth": {"access_token": "   "}}}"#,
        );

        let path: Vec<String> = ["tokens", "oauth", "access_token"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let result = probe_nested_oauth(Some(&dir), &file, &path, "Pi OAuth");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            result.is_none(),
            "whitespace nested token must not be valid auth; got {result:?}"
        );
    }

    #[test]
    fn nested_oauth_with_real_leaf_token_returns_valid() {
        let (dir, file) = write_oauth_file(
            "nested-real",
            r#"{"tokens": {"oauth": {"access_token": "pi-real-token-xyz"}}}"#,
        );

        let path: Vec<String> = ["tokens", "oauth", "access_token"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let result = probe_nested_oauth(Some(&dir), &file, &path, "Pi OAuth");

        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(result, Some(AuthStatus::Valid { .. })));
    }
}

#[cfg(test)]
mod env_probe_tests {
    use super::probe_env_keys;
    use super::AuthStatus;

    fn unique_var(label: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!(
            "HM_TEST_AUTH_PROBE_{}_{}_{}",
            label,
            std::process::id(),
            nanos
        )
    }

    #[test]
    fn unset_env_var_is_not_configured() {
        let var = unique_var("UNSET");
        std::env::remove_var(&var);
        let result = probe_env_keys(std::slice::from_ref(&var), "API key");
        std::env::remove_var(&var);
        assert!(matches!(result, AuthStatus::NotConfigured));
    }

    #[test]
    fn empty_env_var_is_not_valid_auth() {
        let var = unique_var("EMPTY");
        std::env::set_var(&var, "");
        let result = probe_env_keys(std::slice::from_ref(&var), "API key");
        std::env::remove_var(&var);
        assert!(
            matches!(result, AuthStatus::NotConfigured),
            "empty env var must not flip auth to Valid; got {result:?}"
        );
    }

    #[test]
    fn whitespace_only_env_var_is_not_valid_auth() {
        let var = unique_var("WS");
        std::env::set_var(&var, "   \t\n");
        let result = probe_env_keys(std::slice::from_ref(&var), "API key");
        std::env::remove_var(&var);
        assert!(
            matches!(result, AuthStatus::NotConfigured),
            "whitespace-only env var must not flip auth to Valid; got {result:?}"
        );
    }

    #[test]
    fn non_empty_env_var_is_valid_auth() {
        let var = unique_var("VALID");
        std::env::set_var(&var, "real-token-abcdef");
        let result = probe_env_keys(std::slice::from_ref(&var), "API key");
        std::env::remove_var(&var);
        match result {
            AuthStatus::Valid { detail } => {
                assert!(detail.contains(&var), "detail should mention var: {detail}");
                assert!(
                    detail.contains("API key"),
                    "detail should keep label: {detail}"
                );
            }
            other => panic!("expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn first_non_empty_var_wins_over_later_non_empty() {
        let first = unique_var("FIRST");
        let second = unique_var("SECOND");
        std::env::set_var(&first, "first-real");
        std::env::set_var(&second, "second-real");

        let result = probe_env_keys(&[first.clone(), second.clone()], "API key");

        std::env::remove_var(&first);
        std::env::remove_var(&second);

        match result {
            AuthStatus::Valid { detail } => {
                assert!(detail.contains(&first), "first var should win: {detail}");
                assert!(
                    !detail.contains(&second),
                    "later var must not also appear: {detail}"
                );
            }
            other => panic!("expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn empty_first_var_skips_to_non_empty_second() {
        let first = unique_var("EMPTYFIRST");
        let second = unique_var("REALSECOND");
        std::env::set_var(&first, "");
        std::env::set_var(&second, "real-token");

        let result = probe_env_keys(&[first.clone(), second.clone()], "API key");

        std::env::remove_var(&first);
        std::env::remove_var(&second);

        match result {
            AuthStatus::Valid { detail } => {
                assert!(detail.contains(&second), "second var should win: {detail}");
            }
            other => panic!("expected Valid, got {other:?}"),
        }
    }
}
