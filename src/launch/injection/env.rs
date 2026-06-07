use std::collections::HashMap;

use anyhow::Result;

use crate::config::ResolvedProfile;
use crate::runtimes::manifest::EnvInjection;

use super::validation::{effective_endpoint, validate_bearer_value_at_runtime};

pub fn apply_env_strategy(
    spec: &EnvInjection,
    resolved: &ResolvedProfile,
    env: &mut HashMap<String, String>,
) -> Result<()> {
    if let Some(gateway) = resolved.gateway.as_ref() {
        if !gateway.providers.iter().any(|p| p == &spec.provider) {
            anyhow::bail!(
                "profile gateway routes providers [{}] but runtime needs provider '{}' (supported by runtime: [{}])",
                gateway.providers.join(", "),
                spec.provider,
                spec.supported_providers.join(", ")
            );
        }
        if !spec.supported_providers.contains(&spec.provider) {
            anyhow::bail!(
                "runtime provider '{}' not declared in supported_providers [{}]",
                spec.provider,
                spec.supported_providers.join(", ")
            );
        }
        let bearer = gateway.bearer.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "gateway.bearer is required for env injection strategy (set [profiles.<name>.gateway].bearer)"
            )
        })?;
        validate_bearer_value_at_runtime(bearer)?;
        let effective_strip = gateway
            .endpoint_strip_v1_override
            .unwrap_or(spec.endpoint_strip_v1);
        let endpoint = effective_endpoint(&gateway.base_url, effective_strip);

        for key in &spec.strip_envs {
            env.remove(key);
        }
        env.insert(spec.endpoint_env.clone(), endpoint);
        env.insert(spec.api_key_env.clone(), bearer.to_string());
        return Ok(());
    }

    if let Some(bearer) = resolved.bearer.as_deref() {
        validate_bearer_value_at_runtime(bearer)?;
    }

    for key in &spec.strip_envs {
        env.remove(key);
    }
    if let Some(endpoint) = resolved.endpoint.as_deref() {
        let trimmed = effective_endpoint(endpoint, spec.endpoint_strip_v1);
        env.insert(spec.endpoint_env.clone(), trimmed);
    }
    if let Some(bearer) = resolved.bearer.as_deref() {
        env.insert(spec.api_key_env.clone(), bearer.to_string());
    }
    Ok(())
}
