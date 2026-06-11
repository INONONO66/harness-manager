use std::path::Path;

use super::provider::is_valid_pi_credential;
use super::util::to_camel;
pub(super) use crate::runtimes::types::AuthStatus;

pub(super) fn probe_json_file(
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
