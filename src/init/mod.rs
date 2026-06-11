use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;

use crate::harnesses::builtin::BUILTIN_MANIFESTS;
use crate::harnesses::install;
use crate::harnesses::manifest::ManifestPackageSpec;
use crate::harnesses::registry::HarnessRegistry;
use crate::runtimes::builtin::BUILTIN_RUNTIME_MANIFESTS;
use crate::runtimes::registry::RuntimeRegistry;

pub fn run_init(force: bool, install_harnesses: bool) -> Result<()> {
    let runtimes_dir = config_subdir("runtimes.d")?;
    let harnesses_dir = config_subdir("harnesses.d")?;
    fs::create_dir_all(&runtimes_dir)
        .with_context(|| format!("create {}", runtimes_dir.display()))?;
    fs::create_dir_all(&harnesses_dir)
        .with_context(|| format!("create {}", harnesses_dir.display()))?;

    println!(
        "{} {} runtime manifests into {}",
        "Init".green().bold(),
        if force { "force-writing" } else { "writing" },
        runtimes_dir.display()
    );
    let runtime_summary =
        extract_manifests(BUILTIN_RUNTIME_MANIFESTS, &runtimes_dir, force, "runtimes")?;
    runtime_summary.report();

    println!(
        "\n{} {} harness manifests into {}",
        "Init".green().bold(),
        if force { "force-writing" } else { "writing" },
        harnesses_dir.display()
    );
    let harness_summary = extract_manifests(BUILTIN_MANIFESTS, &harnesses_dir, force, "harnesses")?;
    harness_summary.report();

    if install_harnesses {
        println!("\n{} all non-manual harnesses", "Installing".green().bold());
        let runtimes = RuntimeRegistry::load()?;
        let registry = HarnessRegistry::load(&runtimes)?;
        let failed = install_all_harnesses(&registry);
        if failed > 0 {
            anyhow::bail!("{failed} harness install(s) failed during init --install");
        }
    }

    Ok(())
}

fn config_subdir(name: &str) -> Result<PathBuf> {
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no $XDG_CONFIG_HOME or $HOME to anchor ~/.config/hm/{}",
                name
            )
        })?;
    Ok(root.join("hm").join(name))
}

struct InitSummary {
    written: Vec<String>,
    skipped: Vec<String>,
}

impl InitSummary {
    fn report(&self) {
        if self.written.is_empty() && self.skipped.is_empty() {
            println!("  (nothing to do)");
            return;
        }
        for w in &self.written {
            println!("  {} {}", "+".green().bold(), w);
        }
        for s in &self.skipped {
            println!(
                "  {} {} ({})",
                "-".dimmed(),
                s,
                "already exists, kept; use --force to overwrite".dimmed()
            );
        }
        println!(
            "  {} written, {} skipped",
            self.written.len().to_string().green(),
            self.skipped.len().to_string().dimmed()
        );
    }
}

fn extract_manifests(
    sources: &[(&str, &str)],
    dest_dir: &Path,
    force: bool,
    label: &str,
) -> Result<InitSummary> {
    let mut written = Vec::new();
    let mut skipped = Vec::new();
    for (src_path, content) in sources {
        let filename = Path::new(src_path)
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("{}: invalid source path '{}'", label, src_path))?;
        let dest = dest_dir.join(filename);
        if !dest.starts_with(dest_dir) {
            anyhow::bail!(
                "{}: refused to write outside {} (resolved {})",
                label,
                dest_dir.display(),
                dest.display()
            );
        }
        if dest.exists() && !force {
            skipped.push(filename.to_string_lossy().to_string());
            continue;
        }
        fs::write(&dest, content).with_context(|| format!("write {}", dest.display()))?;
        written.push(filename.to_string_lossy().to_string());
    }
    Ok(InitSummary { written, skipped })
}

fn install_all_harnesses(registry: &HarnessRegistry) -> usize {
    let mut succeeded = Vec::new();
    let mut failed: Vec<(String, String)> = Vec::new();
    let mut manual = Vec::new();

    for spec in registry.specs() {
        if matches!(&spec.package, ManifestPackageSpec::Manual { .. }) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_writes_each_manifest_and_skips_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("runtimes.d");
        fs::create_dir_all(&dest).unwrap();

        let sources: &[(&str, &str)] = &[("runtimes/builtin/foo.toml", "schema_version = 1\n")];

        let first = extract_manifests(sources, &dest, false, "runtimes").unwrap();
        assert_eq!(first.written, vec!["foo.toml".to_string()]);
        assert_eq!(first.skipped.len(), 0);
        assert_eq!(
            fs::read_to_string(dest.join("foo.toml")).unwrap(),
            "schema_version = 1\n"
        );

        // Second run without force → skipped, content preserved
        fs::write(dest.join("foo.toml"), "user-edited").unwrap();
        let second = extract_manifests(sources, &dest, false, "runtimes").unwrap();
        assert_eq!(second.skipped, vec!["foo.toml".to_string()]);
        assert_eq!(second.written.len(), 0);
        assert_eq!(
            fs::read_to_string(dest.join("foo.toml")).unwrap(),
            "user-edited"
        );

        // Force run → user edit overwritten
        let third = extract_manifests(sources, &dest, true, "runtimes").unwrap();
        assert_eq!(third.written, vec!["foo.toml".to_string()]);
        assert_eq!(
            fs::read_to_string(dest.join("foo.toml")).unwrap(),
            "schema_version = 1\n"
        );
    }

    #[test]
    fn extract_strips_parent_components_in_source_path() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("runtimes.d");
        fs::create_dir_all(&dest).unwrap();

        let sources: &[(&str, &str)] = &[("evil/../../etc/passwd", "x = 1\n")];
        let summary = extract_manifests(sources, &dest, true, "runtimes").unwrap();

        assert_eq!(summary.written, vec!["passwd".to_string()]);
        assert!(dest.join("passwd").exists());
        assert!(!Path::new("/etc/passwd_dropped_by_test").exists());
    }
}
