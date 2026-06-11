pub(super) use crate::runtimes::types::AuthStatus;

pub(super) fn probe_env_keys(vars: &[String], label: &str) -> AuthStatus {
    for var in vars {
        if std::env::var(var).is_ok_and(|v| !v.trim().is_empty()) {
            return AuthStatus::Valid {
                detail: format!("{} ({})", label, var),
            };
        }
    }
    AuthStatus::NotConfigured
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
