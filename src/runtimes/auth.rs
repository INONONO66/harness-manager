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
        AuthProbeRecord::KeychainHeuristic { marker_file, label } => {
            probe_keychain(config_dir, marker_file, label)
        }
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
        if json.get(existence_field).is_some() || json.get(to_camel(existence_field)).is_some() {
            return Some(AuthStatus::Valid {
                detail: label.to_string(),
            });
        }
        return None;
    }

    Some(AuthStatus::Valid {
        detail: label.to_string(),
    })
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
        .and_then(|v| v.as_str())?;

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

    let token = current.as_str()?;
    Some(token_to_auth_status(token, label))
}

fn probe_data_dir_json(data_subdir: &str, file_name: &str, label: &str) -> Option<AuthStatus> {
    let file = resolve_data_file(data_subdir, file_name)?;
    if !file.is_file() {
        return None;
    }

    let content = std::fs::read_to_string(&file).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    if json.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return None;
    }

    let provider_count = json.as_object().map(|o| o.len()).unwrap_or(0);
    Some(AuthStatus::Valid {
        detail: format!("{} ({} providers)", label, provider_count),
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

fn probe_keychain(config_dir: Option<&Path>, marker_file: &str, label: &str) -> AuthStatus {
    if !cfg!(target_os = "macos") {
        return AuthStatus::NotConfigured;
    }
    let Some(dir) = config_dir else {
        return AuthStatus::NotConfigured;
    };
    if dir.is_dir() && dir.join(marker_file).is_file() {
        return AuthStatus::Valid {
            detail: label.to_string(),
        };
    }
    AuthStatus::NotConfigured
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
