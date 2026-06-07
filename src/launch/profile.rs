use std::collections::HashMap;
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
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    injection::apply_injection(record_injection, resolved, env, &home_dir)
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
