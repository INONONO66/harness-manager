use anyhow::Result;
use colored::Colorize;

use crate::harnesses::install;
use crate::harnesses::registry::HarnessRegistry;
use crate::harnesses::types::PackageSpec;
use crate::runtimes::registry::RuntimeRegistry;

pub fn run_init(install_harnesses: bool) -> Result<()> {
    if !install_harnesses {
        println!(
            "{} runtimes and harnesses are built into hm — nothing to write.\n  Run {} to install every built-in harness.",
            "Init:".green().bold(),
            "hm init --install".cyan()
        );
        return Ok(());
    }

    println!("\n{} all non-manual harnesses", "Installing".green().bold());
    let runtimes = RuntimeRegistry::load()?;
    let registry = HarnessRegistry::load(&runtimes)?;
    let failed = install_all_harnesses(&registry);
    if failed > 0 {
        anyhow::bail!("{failed} harness install(s) failed during init --install");
    }
    Ok(())
}

fn install_all_harnesses(registry: &HarnessRegistry) -> usize {
    let mut succeeded = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut manual = Vec::new();

    for spec in registry.specs() {
        if matches!(&spec.package, PackageSpec::Manual { .. }) {
            manual.push(spec.id.clone());
            continue;
        }
        match install::install(registry, &spec.id) {
            Ok(()) => succeeded.push(spec.id.clone()),
            Err(e) => failed.push((spec.id.clone(), e.to_string())),
        }
    }

    println!("\n{}", "Install Summary".bold());
    println!("{}", "=".repeat(60));
    println!("  {}: {}", "Succeeded".green().bold(), succeeded.len());
    for id in &succeeded {
        println!("    {} {}", "✓".green(), id);
    }
    if !failed.is_empty() {
        println!("  {}: {}", "Failed".red().bold(), failed.len());
        for (id, err) in &failed {
            println!("    {} {} — {}", "✗".red(), id, err);
        }
    }
    if !manual.is_empty() {
        println!("  {}: {}", "Manual (skipped)".yellow().bold(), manual.len());
        for id in &manual {
            println!(
                "    {} {} ({})",
                "-".dimmed(),
                id,
                "package.kind = manual".dimmed()
            );
        }
    }
    failed.len()
}
