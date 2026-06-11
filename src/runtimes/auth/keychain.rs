use std::path::Path;

pub(super) use crate::runtimes::types::AuthStatus;

pub(super) fn probe_keychain(
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
