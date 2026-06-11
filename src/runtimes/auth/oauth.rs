use std::path::Path;

use super::jwt::token_to_auth_status;
use super::util::to_camel;
pub(super) use crate::runtimes::types::AuthStatus;

pub(super) fn probe_oauth_file(
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

pub(super) fn probe_nested_oauth(
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
