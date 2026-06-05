use colored::Colorize;

use crate::config;
use crate::harnesses::registry::HarnessRegistry;
use crate::runtimes;
use crate::runtimes::registry::RUNTIMES;
use crate::runtimes::types::RuntimeSpec;
use crate::secrets;

fn find_runtime_spec(name: &str) -> Option<&'static RuntimeSpec> {
    let lower = name.to_lowercase();
    RUNTIMES
        .iter()
        .find(|r| r.name.to_lowercase() == lower || r.binary_names.iter().any(|b| *b == lower))
}

#[derive(Debug, Clone)]
struct InjectionTarget {
    display_name: String,
    runtime: &'static RuntimeSpec,
}

fn find_injection_target(name: &str, registry: &HarnessRegistry) -> Option<InjectionTarget> {
    if let Some(harness) = registry.find(name) {
        let runtime = find_runtime_spec(&harness.target_runtime)?;
        return Some(InjectionTarget {
            display_name: format!("{} ({})", harness.display_name, runtime.name),
            runtime,
        });
    }
    find_runtime_spec(name).map(|runtime| InjectionTarget {
        display_name: runtime.name.to_string(),
        runtime,
    })
}

pub fn run_inject_plan(
    registry: &HarnessRegistry,
    target: &str,
    profile_name: &str,
) -> anyhow::Result<()> {
    let hm_config = config::load_config()?;
    let resolved = config::resolve_profile(&hm_config, Some(profile_name))?;

    println!(
        "{} for profile '{}'",
        "Injection Plan".bold(),
        resolved.name.cyan()
    );
    println!("{}", "=".repeat(60));

    if let Some(ref desc) = resolved.description {
        println!("  Profile: {}", desc);
    }

    let detected = runtimes::detect_all();

    let targets_to_plan: Vec<InjectionTarget> = if target.to_lowercase() == "all" {
        RUNTIMES
            .iter()
            .filter(|spec| detected.iter().any(|d| d.name == spec.name && d.installed))
            .map(|runtime| InjectionTarget {
                display_name: runtime.name.to_string(),
                runtime,
            })
            .collect()
    } else {
        match find_injection_target(target, registry) {
            Some(spec) => vec![spec],
            None => {
                anyhow::bail!(
                    "unknown runtime or harness: '{}'. Run `hm detect` or `hm harness list` to see available targets.",
                    target
                );
            }
        }
    };

    if targets_to_plan.is_empty() {
        println!("\n{}", "No installed runtimes to plan for.".yellow());
        return Ok(());
    }

    for target in &targets_to_plan {
        let spec = target.runtime;
        println!("\n{}", target.display_name.bold().cyan());
        println!("{}", "-".repeat(40));

        let Some(injection) = spec.injection else {
            println!("  {}", "No injection spec — proxy env only".dimmed());
            print_proxy_plan(&resolved);
            continue;
        };

        println!("  {}", "Strip:".red().bold());
        for var in injection.strip_envs {
            let current = std::env::var(var).ok();
            let status = match &current {
                Some(val) => format!("{} → (removed)", mask_value(val)).red().to_string(),
                None => "(not set)".dimmed().to_string(),
            };
            println!("    {:30} {}", var, status);
        }

        println!("  {}", "Inject:".green().bold());

        if let Some(ref endpoint) = resolved.endpoint {
            let current = std::env::var(injection.endpoint_env).ok();
            println!(
                "    {:30} {} → {}",
                injection.endpoint_env,
                current.as_deref().unwrap_or("(not set)").dimmed(),
                endpoint.green()
            );
        }

        if let Some(ref bearer) = resolved.bearer {
            let current = std::env::var(injection.api_key_env).ok();
            println!(
                "    {:30} {} → {}",
                injection.api_key_env,
                current
                    .map(|v| mask_value(&v))
                    .unwrap_or_else(|| "(not set)".to_string())
                    .dimmed(),
                secrets::mask_secret(bearer).green()
            );
        }

        print_proxy_plan(&resolved);
    }

    println!(
        "\n{}",
        "This is a dry-run. Use `hm use <runtime> --profile <name>` to launch with injection."
            .dimmed()
    );

    Ok(())
}

fn print_proxy_plan(resolved: &config::ResolvedProfile) {
    if resolved.http_proxy.is_some()
        || resolved.https_proxy.is_some()
        || resolved.no_proxy.is_some()
    {
        println!("  {}", "Proxy:".blue().bold());
        if let Some(ref p) = resolved.http_proxy {
            let current = std::env::var("HTTP_PROXY").ok();
            println!(
                "    {:30} {} → {}",
                "HTTP_PROXY",
                current.as_deref().unwrap_or("(not set)").dimmed(),
                p.green()
            );
        }
        if let Some(ref p) = resolved.https_proxy {
            let current = std::env::var("HTTPS_PROXY").ok();
            println!(
                "    {:30} {} → {}",
                "HTTPS_PROXY",
                current.as_deref().unwrap_or("(not set)").dimmed(),
                p.green()
            );
        }
        if let Some(ref p) = resolved.no_proxy {
            let current = std::env::var("NO_PROXY").ok();
            println!(
                "    {:30} {} → {}",
                "NO_PROXY",
                current.as_deref().unwrap_or("(not set)").dimmed(),
                p.green()
            );
        }
    }
}

fn mask_value(val: &str) -> String {
    if val.len() <= 8 {
        return "***".to_string();
    }
    format!("{}...{}", &val[..4], &val[val.len() - 4..])
}

pub fn run_inject_apply(_target: &str, _profile: &str, _persist: bool) -> anyhow::Result<()> {
    eprintln!("{}", "inject apply is not yet implemented. Use `hm use <runtime> --profile <name>` for ephemeral injection.".yellow());
    Ok(())
}

pub fn run_inject_reset(_target: &str) -> anyhow::Result<()> {
    eprintln!("{}", "inject reset is not yet implemented.".yellow());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injection_target_resolves_plugin_harness_id_to_runtime_spec() {
        let registry = crate::harnesses::registry::HarnessRegistry::from_sources(&[
            crate::harnesses::registry::HarnessSource::manifest(
                "inject-plugin.toml",
                r#"
schema_version = 1
id = "inject-plugin"
display_name = "Inject Plugin"
target_runtime = "Codex CLI"
detect_binaries = ["inject-plugin-bin"]
launch_args = []

[package]
kind = "manual"
instructions = "manual"

[isolation]
spoof_home = true
home_subdirs = [".codex"]
static_envs = { CODEX_HOME = "{home}/.codex" }
seed_files = []
"#,
            ),
        ])
        .unwrap();

        let target = find_injection_target("inject-plugin", &registry).unwrap();

        assert_eq!(target.display_name, "Inject Plugin (Codex CLI)");
        assert_eq!(target.runtime.name, "Codex CLI");
    }
}
