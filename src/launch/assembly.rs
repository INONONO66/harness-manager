use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::bail;

use super::effective_profile_name;
use super::profile::{apply_profile, apply_proxy_env};
use super::target::{build_launch_env, resolve_target, runtime_isolation_plan, LaunchTarget};
use crate::config;
use crate::harnesses::detect::package_state_exists;
use crate::harnesses::registry::HarnessRegistry;
use crate::harnesses::types::{HarnessSpec, PackageSpec};
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
    pub auth_state: IsolationAuthState,
}

pub enum IsolationAuthState {
    None,
    ProfileCredentials,
    SharedHostAuth,
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
    let has_profile = resolved_profile.is_some();
    let share_auth_files = !has_profile;
    let iso_setup = prepare_isolation(selected.isolation, selected.runtime, share_auth_files)?;

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

    let binary_override = isolated_package_binary_override(
        selected.isolated_package_binary_path.as_deref(),
        &iso_paths,
        &mut env,
        &selected.binary_names,
    );
    let auth_state = if iso_setup.is_none() {
        IsolationAuthState::None
    } else if has_profile {
        IsolationAuthState::ProfileCredentials
    } else if share_auth_files && runtime_declares_auth_files(selected.runtime) {
        IsolationAuthState::SharedHostAuth
    } else {
        IsolationAuthState::None
    };

    Ok(UseEnvAssembly {
        env,
        binary_names: selected.binary_names,
        display_name: selected.display_name,
        seeded_path,
        launch_args: selected.launch_args,
        profile_applied,
        isolation_present: iso_setup.is_some(),
        binary_override,
        isolated_binary_required: selected.isolated_package_binary_path.is_some(),
        auth_state,
    })
}

fn runtime_declares_auth_files(runtime: &crate::runtimes::manifest::RuntimeRecord) -> bool {
    runtime
        .shared_state
        .as_ref()
        .is_some_and(|plan| !plan.auth_files.is_empty())
}

struct SelectedTarget<'a> {
    runtime: &'a crate::runtimes::manifest::RuntimeRecord,
    isolation: Option<IsolationPlan>,
    binary_names: Vec<String>,
    display_name: String,
    launch_args: Vec<String>,
    isolated_package_binary_path: Option<String>,
}

fn select_target<'a>(
    target: LaunchTarget<'a>,
    allow_keychain: bool,
) -> anyhow::Result<SelectedTarget<'a>> {
    let isolated_package_binary_path = match &target {
        LaunchTarget::Harness { harness, .. } => {
            harness.package.bin_subdir().map(ToString::to_string)
        }
        LaunchTarget::Runtime(_) => None,
    };

    match target {
        LaunchTarget::Runtime(rt) => Ok(SelectedTarget {
            runtime: rt,
            isolation: runtime_isolation_plan(rt, allow_keychain)?,
            binary_names: rt.binary_names.clone(),
            display_name: rt.name.clone(),
            launch_args: Vec::new(),
            isolated_package_binary_path,
        }),
        LaunchTarget::Harness { harness, runtime } => {
            if allow_keychain {
                bail!("--allow-keychain is not supported for harness launches");
            }
            ensure_package_managed_harness_installed(harness)?;
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
                isolated_package_binary_path,
            })
        }
    }
}

fn ensure_package_managed_harness_installed(harness: &HarnessSpec) -> anyhow::Result<()> {
    if matches!(
        harness.package,
        PackageSpec::Custom { .. } | PackageSpec::GitWorktree { .. }
    ) && !package_state_exists(harness)
    {
        bail!(
            "harness '{}' is not installed. Run `hm harness install {}` first.",
            harness.id,
            harness.id
        );
    }
    Ok(())
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
    share_auth_files: bool,
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
    isolation::prepare_runtime_shared_state_with_auth(
        runtime.shared_state.as_ref(),
        &paths,
        share_auth_files,
    )?;
    Ok(Some((iso, paths, lock)))
}

fn isolated_package_binary_override(
    bin_subdir: Option<&str>,
    iso_paths: &Option<isolation::IsolationPaths>,
    env: &mut HashMap<String, String>,
    binary_names: &[String],
) -> Option<PathBuf> {
    let bin_subdir = bin_subdir?;
    let paths = iso_paths.as_ref()?;
    let bin_dir = paths.home.join(bin_subdir);
    prepend_path(env, &bin_dir);
    existing_first_binary(&bin_dir, binary_names)
}

fn prepend_path(env: &mut HashMap<String, String>, bin_dir: &std::path::Path) {
    let bin_dir_str = bin_dir.to_string_lossy().to_string();
    let current_path = env.get("PATH").cloned().unwrap_or_default();
    let new_path = if current_path.is_empty() {
        bin_dir_str.clone()
    } else {
        format!("{bin_dir_str}:{current_path}")
    };
    env.insert("PATH".to_string(), new_path);
}

fn existing_first_binary(bin_dir: &std::path::Path, binary_names: &[String]) -> Option<PathBuf> {
    binary_names.first().and_then(|first_bin| {
        let candidate = bin_dir.join(first_bin);
        candidate.exists().then_some(candidate)
    })
}

#[cfg(test)]
#[path = "assembly_tests.rs"]
mod tests;
