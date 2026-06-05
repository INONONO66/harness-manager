use std::collections::HashMap;

use anyhow::bail;

use crate::harnesses::registry::HarnessRegistry;
use crate::harnesses::types::HarnessSpec;
use crate::isolation;
use crate::isolation::spec::IsolationRecipe;
use crate::runtimes::registry::RUNTIMES;
use crate::runtimes::types::{IsolationSpec, RuntimeSpec};

pub(super) enum LaunchTarget<'a> {
    Runtime(&'static RuntimeSpec),
    Harness {
        harness: &'a HarnessSpec,
        runtime: &'static RuntimeSpec,
    },
}

pub(super) fn find_runtime_spec(name: &str) -> Option<&'static RuntimeSpec> {
    let lower = name.to_lowercase();
    RUNTIMES
        .iter()
        .find(|r| r.name.to_lowercase() == lower || r.binary_names.iter().any(|b| *b == lower))
}

pub(super) fn resolve_target<'a>(
    name: &str,
    registry: &'a HarnessRegistry,
) -> anyhow::Result<LaunchTarget<'a>> {
    if let Some(h) = registry.find(name) {
        let rt = find_runtime_spec(&h.target_runtime).ok_or_else(|| {
            anyhow::anyhow!(
                "harness '{}' targets runtime '{}', but that runtime is not registered",
                h.id,
                h.target_runtime
            )
        })?;
        return Ok(LaunchTarget::Harness {
            harness: h,
            runtime: rt,
        });
    }
    if let Some(rt) = find_runtime_spec(name) {
        return Ok(LaunchTarget::Runtime(rt));
    }
    bail!(
        "unknown target: '{}'. Built-in runtimes: {}. Harnesses: {}. Run `hm detect` or `hm harness list` for status.",
        name,
        runtime_labels(),
        harness_labels(registry)
    )
}

fn runtime_labels() -> String {
    RUNTIMES
        .iter()
        .map(|runtime| runtime.binary_names[0])
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

pub(super) fn runtime_isolation(
    runtime: &'static RuntimeSpec,
    allow_keychain: bool,
) -> anyhow::Result<Option<&'static IsolationSpec>> {
    if allow_keychain {
        return runtime
            .keychain_isolation
            .map(|spec| Some(spec as &IsolationSpec))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "--allow-keychain is not supported for runtime '{}'",
                    runtime.name
                )
            });
    }
    Ok(runtime.isolation)
}

pub(super) fn build_launch_env(
    inherited: &HashMap<String, String>,
    spec: &RuntimeSpec,
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
    if let Some(injection) = spec.injection {
        for var in injection.strip_envs {
            env.remove(*var);
        }
    }
    env
}
