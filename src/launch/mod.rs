use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::bail;
use colored::Colorize;

use crate::config;
use crate::isolation;
use crate::runtimes::types::{IsolationSpec, RuntimeSpec};

mod target;

use target::{build_launch_env, resolve_target, runtime_isolation, LaunchTarget};

pub fn run_use(
    target_name: &str,
    profile_name: Option<&str>,
    print_env: bool,
    allow_keychain: bool,
    extra_args: &[String],
) -> anyhow::Result<()> {
    let target = resolve_target(target_name)?;

    // Destructure into the pieces we need regardless of variant.
    let (spec, effective_isolation, binary_names, display_name): (
        &RuntimeSpec,
        Option<&IsolationSpec>,
        &[&str],
        String,
    ) = match &target {
        LaunchTarget::Runtime(rt) => {
            let iso = runtime_isolation(rt, allow_keychain)?;
            (rt, iso, rt.binary_names, rt.name.to_string())
        }
        LaunchTarget::Harness { harness, runtime } => {
            if allow_keychain {
                bail!("--allow-keychain is not supported for harness launches");
            }
            // Harness always has isolation. The launch binary is either the
            // harness's own wrapper or the underlying runtime's binary.
            let bins: &[&str] = match &harness.launch_binary {
                Some(bin) => std::slice::from_ref(bin),
                None => runtime.binary_names,
            };
            let name = format!("{} ({})", harness.display_name, runtime.name);
            (
                *runtime,
                Some(&harness.isolation as &IsolationSpec),
                bins,
                name,
            )
        }
    };

    // --- Isolation setup ---------------------------------------------------
    let iso_setup = if let Some(iso) = effective_isolation {
        let paths = isolation::IsolationPaths::try_from_spec(iso)?;
        isolation::ensure_isolation_tree(iso, &paths)?;
        isolation::seed_files(iso, &paths)?;
        Some((iso, paths))
    } else {
        None
    };

    // --- Env: start from inherited, then strip + inject --------------------
    let inherited: HashMap<String, String> = std::env::vars().collect();
    let mut env = build_launch_env(
        &inherited,
        spec,
        iso_setup.as_ref().map(|(iso, paths)| (*iso, paths)),
    );

    if let Some((iso, _)) = iso_setup {
        if let Some(caveat) = iso.caveat {
            eprintln!("{} {}", "⚠".yellow().bold(), caveat);
        }
    }

    // --- Profile injection (endpoint, bearer, proxy) -----------------------
    let profile_applied = if let Some(profile_arg) = profile_name {
        let hm_config = config::load_config()?;
        let resolved = config::resolve_profile(&hm_config, Some(profile_arg))?;

        if !print_env {
            eprintln!(
                "{} {} with profile '{}'",
                "Launching".green().bold(),
                display_name.bold(),
                resolved.name.cyan()
            );
        }

        if let Some(injection) = spec.injection {
            if let Some(ref endpoint) = resolved.endpoint {
                let effective_endpoint = if injection.endpoint_strip_v1 {
                    endpoint
                        .trim_end_matches('/')
                        .trim_end_matches("/v1")
                        .to_string()
                } else {
                    endpoint.clone()
                };
                env.insert(injection.endpoint_env.to_string(), effective_endpoint);
            }
            if let Some(ref bearer) = resolved.bearer {
                env.insert(injection.api_key_env.to_string(), bearer.clone());
            }
        }

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
        true
    } else {
        false
    };

    // --- print-env exit path -----------------------------------------------
    if print_env {
        let mut keys: Vec<&String> = env.keys().collect();
        keys.sort();
        for k in keys {
            println!("{}={}", k, env[k]);
        }
        return Ok(());
    }

    // --- Resolve binary and exec -------------------------------------------
    let binary = crate::runtimes::find_binary(binary_names).ok_or_else(|| {
        anyhow::anyhow!(
            "{} is not installed (binary not found in PATH)",
            display_name
        )
    })?;

    if !profile_applied {
        let suffix = if iso_setup.is_some() {
            "(isolated, no profile)"
        } else {
            "(no profile)"
        };
        eprintln!(
            "{} {} {}",
            "Launching".green().bold(),
            display_name.bold(),
            suffix
        );
    }

    let mut cmd = Command::new(&binary);
    cmd.args(extra_args);
    cmd.env_clear();
    for (k, v) in &env {
        cmd.env(k, v);
    }
    let err = cmd.exec();
    bail!("failed to exec {}: {}", binary.display(), err);
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
