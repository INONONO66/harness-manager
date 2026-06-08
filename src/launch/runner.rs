use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::bail;
use colored::Colorize;

use super::target::{resolve_target, LaunchTarget};
use super::{assemble_use_env, UseEnvAssembly};
use crate::harnesses::registry::HarnessRegistry;
use crate::runtimes::registry::RuntimeRegistry;

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

    if print_env {
        print_assembly_env(&assembly);
        return Ok(());
    }

    announce_launch(&assembly);
    emit_caveats(runtimes, harnesses, target_name, allow_keychain);
    exec_assembly(&assembly, extra_args)
}

fn print_assembly_env(assembly: &UseEnvAssembly) {
    let mut keys: Vec<&String> = assembly.env.keys().collect();
    keys.sort();
    for k in keys {
        println!("{}={}", k, assembly.env[k]);
    }
    if let Some(ref seeded) = assembly.seeded_path {
        eprintln!("{}", seeded.display());
    }
}

fn announce_launch(assembly: &UseEnvAssembly) {
    if let Some(ref applied_name) = assembly.profile_applied {
        eprintln!(
            "{} {} with profile '{}'",
            "Launching".green().bold(),
            assembly.display_name.bold(),
            applied_name.cyan()
        );
        print_seeded_path(assembly);
        return;
    }

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
    print_seeded_path(assembly);
}

fn print_seeded_path(assembly: &UseEnvAssembly) {
    if let Some(ref seeded) = assembly.seeded_path {
        eprintln!(
            "{} seeded {}",
            "✓".green().bold(),
            seeded.display().to_string().cyan()
        );
    }
}

fn exec_assembly(assembly: &UseEnvAssembly, extra_args: &[String]) -> anyhow::Result<()> {
    let binary = if let Some(override_path) = assembly.binary_override.as_ref() {
        override_path.clone()
    } else if assembly.isolated_binary_required {
        bail!(
            "{} isolated binary is not installed. Run `hm harness install {}` before launch.",
            assembly.display_name,
            assembly
                .binary_names
                .first()
                .map(String::as_str)
                .unwrap_or("this harness")
        );
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
