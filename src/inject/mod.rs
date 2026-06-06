use colored::Colorize;

use crate::config;
use crate::harnesses::registry::HarnessRegistry;
use crate::launch::injection::{
    apply_env_strategy, validate_codex_config_seed, validate_provider_config_seed,
    ProviderConfigSeedSource,
};
use crate::runtimes;
use crate::runtimes::manifest::{InjectionRecord, RuntimeRecord};
use crate::runtimes::registry::RuntimeRegistry;
use crate::secrets;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct InjectionTarget<'a> {
    display_name: String,
    runtime: &'a RuntimeRecord,
}

fn find_injection_target<'a>(
    name: &str,
    runtimes: &'a RuntimeRegistry,
    harnesses: &HarnessRegistry,
) -> Option<InjectionTarget<'a>> {
    if let Some(harness) = harnesses.find(name) {
        let runtime = runtimes.find_by_display_name(&harness.target_runtime)?;
        return Some(InjectionTarget {
            display_name: format!("{} ({})", harness.display_name, runtime.name),
            runtime,
        });
    }
    runtimes.find(name).map(|runtime| InjectionTarget {
        display_name: runtime.name.clone(),
        runtime,
    })
}

pub fn run_inject_plan(
    runtimes: &RuntimeRegistry,
    harnesses: &HarnessRegistry,
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

    let detected = runtimes::detect_all(runtimes);

    let targets_to_plan: Vec<InjectionTarget> = if target.to_lowercase() == "all" {
        runtimes
            .records()
            .iter()
            .filter(|record| {
                detected
                    .iter()
                    .any(|d| d.name == record.name && d.installed)
            })
            .map(|runtime| InjectionTarget {
                display_name: runtime.name.clone(),
                runtime,
            })
            .collect()
    } else {
        match find_injection_target(target, runtimes, harnesses) {
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
        println!("\n{}", target.display_name.bold().cyan());
        println!("{}", "-".repeat(40));

        let Some(injection) = target.runtime.injection.as_ref() else {
            println!("  {}", "No injection spec — proxy env only".dimmed());
            print_proxy_plan(&resolved);
            continue;
        };

        match injection {
            InjectionRecord::Env(env_spec) => {
                println!("  {}", "Strip:".red().bold());
                for var in &env_spec.strip_envs {
                    let current = std::env::var(var).ok();
                    let status = match &current {
                        Some(val) => format!("{} → (removed)", mask_value(val)).red().to_string(),
                        None => "(not set)".dimmed().to_string(),
                    };
                    println!("    {:30} {}", var, status);
                }

                println!("  {}", "Inject:".green().bold());

                let mut env_preview: HashMap<String, String> = HashMap::new();
                let dry = apply_env_strategy(env_spec, &resolved, &mut env_preview);
                match dry {
                    Ok(()) => {
                        if let Some(endpoint) = env_preview.get(&env_spec.endpoint_env) {
                            println!("    {:30} → {}", env_spec.endpoint_env, endpoint.green());
                        }
                        if let Some(key) = env_preview.get(&env_spec.api_key_env) {
                            println!(
                                "    {:30} → {}",
                                env_spec.api_key_env,
                                secrets::mask_secret(key).green()
                            );
                        }
                    }
                    Err(e) => {
                        println!("    {} {}", "✗".red(), e);
                    }
                }
            }
            InjectionRecord::ProviderConfigSeed(seed_spec) => {
                println!("  {}", "Seed:".green().bold());
                let expanded = seed_spec.config_path.replace("{home}", "<isolation-home>");
                println!("    file: {}", expanded.cyan());
                println!("    root: {}", seed_spec.root_key);
                match validate_provider_config_seed(seed_spec, &resolved) {
                    Ok(preview) => {
                        let source_label = match preview.source {
                            ProviderConfigSeedSource::Gateway => "gateway",
                            ProviderConfigSeedSource::LegacyLlm => {
                                "legacy llm → single-provider seed"
                            }
                        };
                        println!("    source: {}", source_label.cyan());
                        println!("    endpoint: {}", preview.endpoint.green());
                        println!("    providers:");
                        for p in &preview.providers {
                            println!("      {} {}", "✓".green(), p);
                        }
                    }
                    Err(e) => {
                        println!("    {} {}", "✗".red(), e);
                    }
                }
            }
            InjectionRecord::CodexConfigSeed(codex_spec) => {
                println!("  {}", "Strip:".red().bold());
                for var in &codex_spec.strip_envs {
                    let current = std::env::var(var).ok();
                    let status = match &current {
                        Some(val) => format!("{} → (removed)", mask_value(val)).red().to_string(),
                        None => "(not set)".dimmed().to_string(),
                    };
                    println!("    {:30} {}", var, status);
                }

                println!("  {}", "Seed:".green().bold());
                match validate_codex_config_seed(codex_spec, &resolved) {
                    Ok(preview) => {
                        println!("    file: {}", preview.config_path_display.cyan());
                        let source_label = match preview.source {
                            ProviderConfigSeedSource::Gateway => "gateway",
                            ProviderConfigSeedSource::LegacyLlm => {
                                "legacy llm → single-provider seed"
                            }
                        };
                        println!("    source: {}", source_label.cyan());
                        println!("    endpoint: {}", preview.endpoint.green());
                        println!("    provider: {} {}", "✓".green(), preview.provider);
                        println!("    top_level writes:");
                        for (k, v) in &preview.top_level_writes {
                            println!("      {} = {}", k, v.green());
                        }
                        println!("  {}", "Inject:".green().bold());
                        let bearer = resolved
                            .gateway
                            .as_ref()
                            .and_then(|gw| gw.bearer.as_deref())
                            .or(resolved.bearer.as_deref())
                            .unwrap_or("");
                        println!(
                            "    {:30} → {}",
                            preview.api_key_env,
                            secrets::mask_secret(bearer).green()
                        );
                    }
                    Err(e) => {
                        println!("    {} {}", "✗".red(), e);
                    }
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injection_target_resolves_plugin_harness_id_to_runtime_spec() {
        let runtimes = RuntimeRegistry::builtin_only().unwrap();
        let harnesses = HarnessRegistry::from_sources(
            &[crate::harnesses::registry::HarnessSource::manifest(
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
            )],
            &runtimes,
        )
        .unwrap();

        let target = find_injection_target("inject-plugin", &runtimes, &harnesses).unwrap();

        assert_eq!(target.display_name, "Inject Plugin (Codex CLI)");
        assert_eq!(target.runtime.name, "Codex CLI");
    }
}
