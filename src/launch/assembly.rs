use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::bail;

use super::effective_profile_name;
use super::profile::{apply_profile, apply_proxy_env};
use super::target::{build_launch_env, resolve_target, runtime_isolation_plan, LaunchTarget};
use crate::config;
use crate::harnesses::registry::HarnessRegistry;
use crate::harnesses::types::PackageSpec;
use crate::isolation;
use crate::isolation::spec::{IsolationPlan, IsolationRecipe};
use crate::runtimes::registry::RuntimeRegistry;

pub struct UseEnvAssembly {
    pub env: HashMap<String, String>,
    pub binary_names: Vec<String>,
    pub display_name: String,
    pub seeded_path: Option<PathBuf>,
    pub launch_args: Vec<String>,
    pub profile_applied: Option<String>,
    pub isolation_present: bool,
    pub binary_override: Option<PathBuf>,
    pub isolated_binary_required: bool,
}

pub fn assemble_use_env(
    runtimes: &RuntimeRegistry,
    harnesses: &HarnessRegistry,
    target_name: &str,
    profile_name: Option<&str>,
    allow_keychain: bool,
    inherited: HashMap<String, String>,
) -> anyhow::Result<UseEnvAssembly> {
    let target = resolve_target(target_name, runtimes, harnesses)?;
    let selected = select_target(target, allow_keychain)?;
    let resolved_profile = resolve_profile_before_isolation(profile_name)?;
    let iso_setup = prepare_isolation(selected.isolation, selected.runtime)?;

    let mut env = build_launch_env(
        &inherited,
        selected.runtime,
        iso_setup
            .as_ref()
            .map(|(iso, paths, _lock)| (iso as &dyn IsolationRecipe, paths)),
    );

    let iso_paths = iso_setup.as_ref().map(|(_, paths, _)| paths.clone());
    let (profile_applied, seeded_path) = match resolved_profile {
        Some(resolved) => {
            let seeded = apply_profile(&resolved, selected.runtime, &mut env, iso_paths.as_ref())?;
            apply_proxy_env(&resolved, &mut env);
            (Some(resolved.name.clone()), seeded)
        }
        None => (None, None),
    };

    let binary_override = npm_binary_override(
        selected.npm_isolated_harness,
        &iso_paths,
        &mut env,
        &selected.binary_names,
    );

    Ok(UseEnvAssembly {
        env,
        binary_names: selected.binary_names,
        display_name: selected.display_name,
        seeded_path,
        launch_args: selected.launch_args,
        profile_applied,
        isolation_present: iso_setup.is_some(),
        binary_override,
        isolated_binary_required: selected.npm_isolated_harness,
    })
}

struct SelectedTarget<'a> {
    runtime: &'a crate::runtimes::manifest::RuntimeRecord,
    isolation: Option<IsolationPlan>,
    binary_names: Vec<String>,
    display_name: String,
    launch_args: Vec<String>,
    npm_isolated_harness: bool,
}

fn select_target<'a>(
    target: LaunchTarget<'a>,
    allow_keychain: bool,
) -> anyhow::Result<SelectedTarget<'a>> {
    let npm_isolated_harness = matches!(
        &target,
        LaunchTarget::Harness { harness, .. }
            if matches!(&harness.package, PackageSpec::NpmIsolated { .. })
    );

    match target {
        LaunchTarget::Runtime(rt) => Ok(SelectedTarget {
            runtime: rt,
            isolation: runtime_isolation_plan(rt, allow_keychain)?,
            binary_names: rt.binary_names.clone(),
            display_name: rt.name.clone(),
            launch_args: Vec::new(),
            npm_isolated_harness,
        }),
        LaunchTarget::Harness { harness, runtime } => {
            if allow_keychain {
                bail!("--allow-keychain is not supported for harness launches");
            }
            let binary_names = match &harness.launch_binary {
                Some(bin) => vec![bin.clone()],
                None => runtime.binary_names.clone(),
            };
            Ok(SelectedTarget {
                runtime,
                isolation: Some(harness.isolation.clone()),
                binary_names,
                display_name: format!("{} ({})", harness.display_name, runtime.name),
                launch_args: harness.launch_args.clone(),
                npm_isolated_harness,
            })
        }
    }
}

fn resolve_profile_before_isolation(
    profile_name: Option<&str>,
) -> anyhow::Result<Option<crate::config::ResolvedProfile>> {
    let hm_config = config::load_config()?;
    match effective_profile_name(profile_name, &hm_config) {
        Some(name) => Ok(Some(config::resolve_profile(&hm_config, Some(&name))?)),
        None => Ok(None),
    }
}

fn prepare_isolation(
    isolation_plan: Option<IsolationPlan>,
    runtime: &crate::runtimes::manifest::RuntimeRecord,
) -> anyhow::Result<
    Option<(
        IsolationPlan,
        isolation::IsolationPaths,
        isolation::IsolationLockGuard,
    )>,
> {
    let Some(iso) = isolation_plan else {
        return Ok(None);
    };
    let paths = isolation::IsolationPaths::try_from_spec(&iso)?;
    let lock = isolation::IsolationLockGuard::acquire(&paths)?;
    isolation::ensure_isolation_tree(&iso, &paths)?;
    isolation::seed_files(&iso, &paths)?;
    isolation::link_main_runtime_databases(&runtime.name, &paths)?;
    Ok(Some((iso, paths, lock)))
}

fn npm_binary_override(
    enabled: bool,
    iso_paths: &Option<isolation::IsolationPaths>,
    env: &mut HashMap<String, String>,
    binary_names: &[String],
) -> Option<PathBuf> {
    if !enabled {
        return None;
    }
    let paths = iso_paths.as_ref()?;
    let bin_dir = paths.home.join(".npm").join("bin");
    let bin_dir_str = bin_dir.to_string_lossy().to_string();
    let current_path = env.get("PATH").cloned().unwrap_or_default();
    let new_path = if current_path.is_empty() {
        bin_dir_str.clone()
    } else {
        format!("{bin_dir_str}:{current_path}")
    };
    env.insert("PATH".to_string(), new_path);
    binary_names.first().and_then(|first_bin| {
        let candidate = bin_dir.join(first_bin);
        candidate.exists().then_some(candidate)
    })
}
