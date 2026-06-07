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
        AuthProbeRecord::KeychainHeuristic {
            marker_file,
            keychain_service,
            label,
        } => probe_keychain(config_dir, marker_file, keychain_service, label),
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

    let any_meaningful = json
        .as_object()
        .map(|obj| obj.values().any(is_meaningful_json_value))
        .unwrap_or(false);
    if !any_meaningful {
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
    if !file.is_file() {
        return None;
    }
    let content = std::fs::read_to_string(file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let object = json.as_object()?;
    let meaningful_count = object
        .values()
        .filter(|v| is_meaningful_json_value(v))
        .count();
    if meaningful_count == 0 {
        return None;
    }
    Some(AuthStatus::Valid {
        detail: format!("{} ({} providers)", label, meaningful_count),
    })
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

fn probe_keychain(
    config_dir: Option<&Path>,
    marker_file: &str,
    keychain_service: &str,
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
    if !keychain_item_exists(keychain_service) {
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
    fn json_file_no_existence_field_with_one_real_value_returns_valid() {
        let (dir, file) = write_json("no-field-real", r#"{"token": "real-secret"}"#);
        let result = probe_json_file(Some(&dir), &file, "", "Pi token");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(result, Some(AuthStatus::Valid { .. })),
            "no-existence-field probe must accept a meaningful value; got {result:?}"
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
        let result = probe_keychain(Some(&dir), "settings.json", &service, "OAuth (Keychain)");
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
        let result = probe_keychain(Some(&dir), "settings.json", &service, "OAuth (Keychain)");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(result, AuthStatus::NotConfigured));
    }

    #[test]
    fn keychain_probe_no_config_dir_returns_not_configured() {
        let service = unique_service("no-dir");
        let result = probe_keychain(None, "settings.json", &service, "OAuth (Keychain)");
        assert!(matches!(result, AuthStatus::NotConfigured));
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn keychain_probe_on_non_macos_is_not_configured_regardless_of_marker() {
        let dir = unique_dir("non-mac");
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let result = probe_keychain(
            Some(&dir),
            "settings.json",
            "any-service",
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
    fn data_dir_one_real_among_nulls_counts_only_meaningful() {
        let file = write_provider_file(
            "mixed",
            r#"{"openai": null, "anthropic": {"type": "api", "key": "real"}, "google": null}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("(1 providers)"),
                    "expected '(1 providers)' counting only meaningful entries, got: {detail}"
                );
            }
            other => panic!("expected Valid with count, got {other:?}"),
        }
    }

    #[test]
    fn data_dir_multiple_real_providers_counted() {
        let file = write_provider_file(
            "two-real",
            r#"{"openai": {"key": "k1"}, "anthropic": {"key": "k2"}, "stale": null}"#,
        );
        let result = probe_data_dir_json_at(&file, "Provider auth");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        match result {
            Some(AuthStatus::Valid { detail }) => {
                assert!(
                    detail.contains("(2 providers)"),
                    "expected '(2 providers)' from 2 real and 1 null, got: {detail}"
                );
            }
            other => panic!("expected Valid with count, got {other:?}"),
        }
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
