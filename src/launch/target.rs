use std::collections::HashMap;

use anyhow::bail;

use crate::harnesses::registry::HarnessRegistry;
use crate::harnesses::types::HarnessSpec;
use crate::isolation;
use crate::isolation::spec::{IsolationPlan, IsolationRecipe};
use crate::runtimes::manifest::RuntimeRecord;
use crate::runtimes::registry::RuntimeRegistry;

pub(super) enum LaunchTarget<'a> {
    Runtime(&'a RuntimeRecord),
    Harness {
        harness: &'a HarnessSpec,
        runtime: &'a RuntimeRecord,
    },
}

pub(super) fn resolve_target<'a>(
    name: &str,
    runtimes: &'a RuntimeRegistry,
    harnesses: &'a HarnessRegistry,
) -> anyhow::Result<LaunchTarget<'a>> {
    if let Some(harness) = harnesses.find(name) {
        let runtime = runtimes
            .find_by_display_name(&harness.target_runtime)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "harness '{}' targets runtime '{}', but that runtime is not registered",
                    harness.id,
                    harness.target_runtime
                )
            })?;
        return Ok(LaunchTarget::Harness { harness, runtime });
    }
    if let Some(runtime) = runtimes.find(name) {
        return Ok(LaunchTarget::Runtime(runtime));
    }
    bail!(
        "unknown target: '{}'. Built-in runtimes: {}. Harnesses: {}. Run `hm detect` or `hm harness list` for status.",
        name,
        runtime_labels(runtimes),
        harness_labels(harnesses)
    )
}

fn runtime_labels(runtimes: &RuntimeRegistry) -> String {
    runtimes
        .records()
        .iter()
        .filter_map(|runtime| runtime.binary_names.first().cloned())
        .collect::<Vec<_>>()
        .join(", ")
}

fn harness_labels(registry: &HarnessRegistry) -> String {
    let labels = registry
        .specs()
        .iter()
        .map(|harness| {
            if harness.aliases.is_empty() {
                harness.id.clone()
            } else {
                format!("{} ({})", harness.id, harness.aliases.join(", "))
            }
        })
        .collect::<Vec<_>>();
    if labels.is_empty() {
        "none registered".to_string()
    } else {
        labels.join(", ")
    }
}

pub(super) fn runtime_isolation_plan(
    runtime: &RuntimeRecord,
    allow_keychain: bool,
) -> anyhow::Result<Option<IsolationPlan>> {
    if allow_keychain {
        return runtime.keychain_isolation.clone().map(Some).ok_or_else(|| {
            anyhow::anyhow!(
                "--allow-keychain is not supported for runtime '{}'",
                runtime.name
            )
        });
    }
    Ok(runtime.isolation.clone())
}

pub(super) fn build_launch_env(
    inherited: &HashMap<String, String>,
    runtime: &RuntimeRecord,
    iso_setup: Option<(&dyn IsolationRecipe, &isolation::IsolationPaths)>,
) -> HashMap<String, String> {
    let mut env = if let Some((iso, paths)) = iso_setup {
        isolation::build_sanitized_isolation_env(inherited, iso, paths)
    } else {
        let mut env = inherited.clone();
        for var in isolation::GLOBAL_AI_STRIP {
            env.remove(*var);
        }
        env
    };
    if let Some(injection) = runtime.injection.as_ref() {
        for var in injection_strip_envs(injection) {
            env.remove(var);
        }
    }
    env
}

fn injection_strip_envs(injection: &crate::runtimes::manifest::InjectionRecord) -> Vec<&str> {
    use crate::runtimes::manifest::InjectionRecord;
    match injection {
        InjectionRecord::Env(env) => env.strip_envs.iter().map(String::as_str).collect(),
        InjectionRecord::ProviderConfigSeed(_) => Vec::new(),
        InjectionRecord::CodexConfigSeed(spec) => {
            spec.strip_envs.iter().map(String::as_str).collect()
        }
    }
}
