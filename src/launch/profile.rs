use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

use crate::config::ResolvedProfile;
use crate::isolation;
use crate::launch::injection;
use crate::runtimes::manifest::RuntimeRecord;

pub(super) fn apply_profile(
    resolved: &ResolvedProfile,
    runtime: &RuntimeRecord,
    env: &mut HashMap<String, String>,
    iso_paths: Option<&isolation::IsolationPaths>,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(record_injection) = runtime.injection.as_ref() else {
        return Ok(None);
    };
    let home_dir = iso_paths
        .map(|paths| paths.home.clone())
        .map(Ok)
        .unwrap_or_else(|| non_isolated_profile_home(std::env::var_os("HOME")))?;
    injection::apply_injection(record_injection, resolved, env, &home_dir)
}

fn non_isolated_profile_home(home: Option<OsString>) -> anyhow::Result<PathBuf> {
    home.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("cannot resolve home directory for profile injection"))
}

pub(super) fn apply_proxy_env(resolved: &ResolvedProfile, env: &mut HashMap<String, String>) {
    if let Some(ref proxy) = resolved.http_proxy {
        env.insert("HTTP_PROXY".to_string(), proxy.clone());
        env.insert("http_proxy".to_string(), proxy.clone());
    }
    if let Some(ref proxy) = resolved.https_proxy {
        env.insert("HTTPS_PROXY".to_string(), proxy.clone());
        env.insert("https_proxy".to_string(), proxy.clone());
    }
    if let Some(ref no_proxy) = resolved.no_proxy {
        env.insert("NO_PROXY".to_string(), no_proxy.clone());
        env.insert("no_proxy".to_string(), no_proxy.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_injection_home_errors_when_home_env_is_missing() {
        // Given: non-isolated profile injection has no HOME environment value.
        let home = None;

        // When: hm resolves the profile injection home.
        let err = non_isolated_profile_home(home).unwrap_err();

        // Then: it fails closed instead of falling back to the current directory.
        assert!(
            err.to_string().contains("home directory"),
            "expected home resolution error: {err:#}"
        );
    }

    #[test]
    fn profile_injection_home_errors_when_home_env_is_empty() {
        // Given: non-isolated profile injection has an empty HOME environment value.
        let home = Some(std::ffi::OsString::new());

        // When: hm resolves the profile injection home.
        let err = non_isolated_profile_home(home).unwrap_err();

        // Then: it rejects the unusable home value.
        assert!(
            err.to_string().contains("home directory"),
            "expected home resolution error: {err:#}"
        );
    }
}
