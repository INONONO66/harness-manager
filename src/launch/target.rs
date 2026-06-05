use std::collections::HashMap;

use anyhow::bail;

use crate::harnesses;
use crate::harnesses::types::HarnessSpec;
use crate::isolation;
use crate::runtimes::registry::{CLAUDE_KEYCHAIN_ISOLATION, RUNTIMES};
use crate::runtimes::types::{IsolationSpec, RuntimeSpec};

pub(super) enum LaunchTarget {
    Runtime(&'static RuntimeSpec),
    Harness {
        harness: &'static HarnessSpec,
        runtime: &'static RuntimeSpec,
    },
}

pub(super) fn find_runtime_spec(name: &str) -> Option<&'static RuntimeSpec> {
    let lower = name.to_lowercase();
    RUNTIMES
        .iter()
        .find(|r| r.name.to_lowercase() == lower || r.binary_names.iter().any(|b| *b == lower))
}

pub(super) fn resolve_target(name: &str) -> anyhow::Result<LaunchTarget> {
    if let Some(h) = harnesses::find_harness_spec(name) {
        let rt = find_runtime_spec(h.target_runtime).ok_or_else(|| {
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
        "unknown target: '{}'. Run `hm detect` or `hm harness list` to see available targets.",
        name
    )
}

pub(super) fn runtime_isolation(
    runtime: &'static RuntimeSpec,
    allow_keychain: bool,
) -> anyhow::Result<Option<&'static IsolationSpec>> {
    if allow_keychain {
        if runtime.name != "Claude Code" {
            bail!("--allow-keychain is only supported for Claude Code");
        }
        return Ok(Some(&CLAUDE_KEYCHAIN_ISOLATION as &IsolationSpec));
    }
    Ok(runtime.isolation)
}

pub(super) fn build_launch_env(
    inherited: &HashMap<String, String>,
    spec: &RuntimeSpec,
    iso_setup: Option<(&IsolationSpec, &isolation::IsolationPaths)>,
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
