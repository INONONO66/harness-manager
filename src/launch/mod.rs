use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::bail;
use colored::Colorize;

use crate::config::{self, HmConfig, ResolvedProfile};
use crate::harnesses::registry::HarnessRegistry;
use crate::harnesses::types::PackageSpec;
use crate::isolation;
use crate::isolation::spec::IsolationRecipe;
use crate::runtimes::manifest::RuntimeRecord;
use crate::runtimes::registry::RuntimeRegistry;

pub mod injection;
mod target;

use target::{build_launch_env, resolve_target, runtime_isolation_plan, LaunchTarget};

pub struct UseEnvAssembly {
    pub env: HashMap<String, String>,
    pub binary_names: Vec<String>,
    pub display_name: String,
    pub seeded_path: Option<PathBuf>,
    pub launch_args: Vec<String>,
    pub profile_applied: Option<String>,
    pub isolation_present: bool,
    pub binary_override: Option<PathBuf>,
}

pub fn effective_profile_name(arg: Option<&str>, hm_config: &HmConfig) -> Option<String> {
    arg.map(str::to_string)
        .or_else(|| hm_config.default_profile.clone())
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

    let npm_isolated_harness = matches!(
        &target,
        LaunchTarget::Harness { harness, .. }
            if matches!(&harness.package, PackageSpec::NpmIsolated { .. })
    );

    let (runtime, effective_isolation, binary_names, display_name, launch_args) = match &target {
        LaunchTarget::Runtime(rt) => {
            let iso = runtime_isolation_plan(rt, allow_keychain)?;
            (
                *rt,
                iso,
                rt.binary_names.clone(),
                rt.name.clone(),
                Vec::new(),
            )
        }
        LaunchTarget::Harness { harness, runtime } => {
            if allow_keychain {
                bail!("--allow-keychain is not supported for harness launches");
            }
            let bins = match &harness.launch_binary {
                Some(bin) => vec![bin.clone()],
                None => runtime.binary_names.clone(),
            };
            let name = format!("{} ({})", harness.display_name, runtime.name);
            (
                *runtime,
                Some(harness.isolation.clone()),
                bins,
                name,
                harness.launch_args.clone(),
            )
        }
    };

    // Ordering contract (do not reorder): profile resolution MUST come
    // before isolation setup. A broken ~/.config/hm/config.toml (parse
    // failure, missing secret file, unknown profile name, etc.) then
    // fails closed without creating isolation directories, lock files,
    // or seed files on disk.
    let hm_config = config::load_config()?;
    let resolved_profile = match effective_profile_name(profile_name, &hm_config) {
        Some(name) => Some(config::resolve_profile(&hm_config, Some(&name))?),
        None => None,
    };

    let iso_setup = if let Some(iso) = effective_isolation {
        let paths = isolation::IsolationPaths::try_from_spec(&iso)?;
        let lock = isolation::IsolationLockGuard::acquire(&paths)?;
        isolation::ensure_isolation_tree(&iso, &paths)?;
        isolation::seed_files(&iso, &paths)?;
        Some((iso, paths, lock))
    } else {
        None
    };

    let mut env = build_launch_env(
        &inherited,
        runtime,
        iso_setup
            .as_ref()
            .map(|(iso, paths, _lock)| (iso as &dyn IsolationRecipe, paths)),
    );

    let iso_paths = iso_setup.as_ref().map(|(_, paths, _)| paths.clone());

    let (profile_applied, seeded_path) = match resolved_profile {
        Some(resolved) => {
            let seeded = apply_profile(&resolved, runtime, &mut env, iso_paths.as_ref())?;
            apply_proxy_env(&resolved, &mut env);
            (Some(resolved.name.clone()), seeded)
        }
        None => (None, None),
    };

    let binary_override = if npm_isolated_harness {
        if let Some(paths) = iso_paths.as_ref() {
            let bin_dir = paths.home.join(".npm").join("bin");
            let bin_dir_str = bin_dir.to_string_lossy().to_string();
            let current_path = env.get("PATH").cloned().unwrap_or_default();
            let new_path = if current_path.is_empty() {
                bin_dir_str.clone()
            } else {
                format!("{}:{}", bin_dir_str, current_path)
            };
            env.insert("PATH".to_string(), new_path);
            binary_names.first().and_then(|first_bin| {
                let candidate = bin_dir.join(first_bin);
                if candidate.exists() {
                    Some(candidate)
                } else {
                    None
                }
            })
        } else {
            None
        }
    } else {
        None
    };

    Ok(UseEnvAssembly {
        env,
        binary_names,
        display_name,
        seeded_path,
        launch_args,
        profile_applied,
        isolation_present: iso_setup.is_some(),
        binary_override,
    })
}

fn apply_profile(
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

fn apply_proxy_env(resolved: &ResolvedProfile, env: &mut HashMap<String, String>) {
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

pub fn run_use(
    runtimes: &RuntimeRegistry,
    harnesses: &HarnessRegistry,
    target_name: &str,
    profile_name: Option<&str>,
    print_env: bool,
    allow_keychain: bool,
    extra_args: &[String],
) -> anyhow::Result<()> {
    let inherited: HashMap<String, String> = std::env::vars().collect();
    let assembly = assemble_use_env(
        runtimes,
        harnesses,
        target_name,
        profile_name,
        allow_keychain,
        inherited,
    )?;

    if !print_env {
        if let Some(ref applied_name) = assembly.profile_applied {
            eprintln!(
                "{} {} with profile '{}'",
                "Launching".green().bold(),
                assembly.display_name.bold(),
                applied_name.cyan()
            );
        } else {
            let suffix = if assembly.isolation_present {
                "(isolated, no profile)"
            } else {
                "(no profile)"
            };
            eprintln!(
                "{} {} {}",
                "Launching".green().bold(),
                assembly.display_name.bold(),
                suffix
            );
        }
        if let Some(ref seeded) = assembly.seeded_path {
            eprintln!(
                "{} seeded {}",
                "✓".green().bold(),
                seeded.display().to_string().cyan()
            );
        }
        emit_caveats(runtimes, harnesses, target_name, allow_keychain);
    }

    if print_env {
        let mut keys: Vec<&String> = assembly.env.keys().collect();
        keys.sort();
        for k in keys {
            println!("{}={}", k, assembly.env[k]);
        }
        if let Some(ref seeded) = assembly.seeded_path {
            eprintln!("{}", seeded.display());
        }
        return Ok(());
    }

    let binary = if let Some(override_path) = assembly.binary_override.as_ref() {
        override_path.clone()
    } else {
        let binary_name_refs: Vec<&str> =
            assembly.binary_names.iter().map(String::as_str).collect();
        crate::runtimes::find_binary(&binary_name_refs).ok_or_else(|| {
            anyhow::anyhow!(
                "{} is not installed (binary not found in PATH)",
                assembly.display_name
            )
        })?
    };

    let mut cmd = Command::new(&binary);
    cmd.args(&assembly.launch_args);
    cmd.args(extra_args);
    cmd.env_clear();
    for (k, v) in &assembly.env {
        cmd.env(k, v);
    }
    let err = cmd.exec();
    bail!("failed to exec {}: {}", binary.display(), err);
}

fn emit_caveats(
    runtimes: &RuntimeRegistry,
    harnesses: &HarnessRegistry,
    target_name: &str,
    allow_keychain: bool,
) {
    let caveat = match resolve_target(target_name, runtimes, harnesses) {
        Ok(LaunchTarget::Runtime(rt)) => {
            if allow_keychain {
                rt.keychain_isolation
                    .as_ref()
                    .and_then(|i| i.caveat.clone())
            } else {
                rt.isolation.as_ref().and_then(|i| i.caveat.clone())
            }
        }
        Ok(LaunchTarget::Harness { harness, .. }) => harness.isolation.caveat.clone(),
        Err(_) => None,
    };
    if let Some(c) = caveat {
        eprintln!("{} {}", "⚠".yellow().bold(), c);
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
