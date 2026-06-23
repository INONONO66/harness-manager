use std::path::{Path, PathBuf};

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use super::registry::HarnessRegistry;
use super::state::has_package_state;
use super::types::{HarnessSpec, PackageSpec};
use crate::isolation::IsolationPaths;

#[derive(Debug)]
pub struct DetectedHarness {
    pub id: String,
    pub aliases: Vec<String>,
    pub display_name: String,
    pub target_runtime: String,
    pub installed: bool,
    pub binary_path: Option<PathBuf>,
    pub package_source: String,
    pub wraps_target_runtime_binary: bool,
}

pub fn detect_one(spec: &HarnessSpec) -> DetectedHarness {
    let binary_names: Vec<&str> = spec.detect_binaries.iter().map(String::as_str).collect();
    let binary = match &spec.package {
        PackageSpec::Custom {
            bin_subdir: Some(subdir),
            ..
        } => detect_in_isolation_home(spec, subdir).filter(|_| package_state_exists(spec)),
        PackageSpec::Custom { .. } | PackageSpec::GitWorktree { .. } => {
            if package_state_exists(spec) {
                crate::runtimes::find_binary(&binary_names)
            } else {
                None
            }
        }
        package if package.bin_subdir().is_some() => package
            .bin_subdir()
            .and_then(|subdir| detect_in_isolation_home(spec, subdir)),
        PackageSpec::NpxInstaller { .. } | PackageSpec::BunxInstaller { .. } => {
            let cache_hit = IsolationPaths::try_from_spec(&spec.isolation)
                .ok()
                .and_then(|paths| package_cache_installed_at(&paths.home, &spec.package));
            cache_hit.and_then(|_| crate::runtimes::find_binary(&binary_names))
        }
        _ => crate::runtimes::find_binary(&binary_names),
    };
    DetectedHarness {
        id: spec.id.clone(),
        aliases: spec.aliases.clone(),
        display_name: spec.display_name.clone(),
        target_runtime: spec.target_runtime.clone(),
        installed: binary.is_some(),
        binary_path: binary,
        package_source: format_package_source(&spec.package),
        wraps_target_runtime_binary: matches!(
            &spec.package,
            PackageSpec::NpxInstaller { .. } | PackageSpec::BunxInstaller { .. }
        ),
    }
}

pub fn package_state_exists(spec: &HarnessSpec) -> bool {
    IsolationPaths::try_from_spec(&spec.isolation)
        .ok()
        .is_some_and(|paths| has_package_state(&paths))
}

fn package_cache_installed_at(home: &Path, package: &PackageSpec) -> Option<PathBuf> {
    match package {
        PackageSpec::NpxInstaller { package, .. } => {
            let npx_root = home.join(".npm").join("_npx");
            let entries = std::fs::read_dir(&npx_root).ok()?;
            for entry in entries.flatten() {
                let candidate = entry.path().join("node_modules").join(package);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
            None
        }
        PackageSpec::BunxInstaller { package, .. } => {
            let versioned_prefix = format!("{package}@");
            for cache_root in [
                home.join(".bun").join("install").join("cache"),
                home.join(".cache")
                    .join(".bun")
                    .join("install")
                    .join("cache"),
            ] {
                let Ok(entries) = std::fs::read_dir(cache_root) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str == *package || name_str.starts_with(&versioned_prefix) {
                        return Some(entry.path());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn format_package_source(pkg: &PackageSpec) -> String {
    match pkg {
        PackageSpec::NpmGlobal { package, .. } => format!("npm-global ({package})"),
        PackageSpec::NpmIsolated { package, .. } => format!("npm-isolated ({package})"),
        PackageSpec::NpxInstaller { package, .. } => format!("npx-installer ({package})"),
        PackageSpec::BunxInstaller { package, .. } => format!("bunx-installer ({package})"),
        PackageSpec::PythonTool { package, .. } => format!("python-tool ({package})"),
        PackageSpec::Custom { .. } => "custom".to_string(),
        PackageSpec::GitWorktree { repository, .. } => format!("git-worktree ({repository})"),
        PackageSpec::Manual { .. } => "manual".to_string(),
    }
}

fn detect_in_isolation_home(spec: &HarnessSpec, bin_subdir: &str) -> Option<PathBuf> {
    let paths = IsolationPaths::try_from_spec(&spec.isolation).ok()?;
    let iso_bin = paths.home.join(bin_subdir);
    spec.detect_binaries.iter().find_map(|name| {
        let candidate = iso_bin.join(name);
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    })
}

pub fn detect_all(registry: &HarnessRegistry) -> Vec<DetectedHarness> {
    registry.specs().iter().map(detect_one).collect()
}

pub fn render_table(detected: &[DetectedHarness]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Harness").fg(Color::White),
            Cell::new("Aliases").fg(Color::White),
            Cell::new("Display Name").fg(Color::White),
            Cell::new("Target Runtime").fg(Color::White),
            Cell::new("Status").fg(Color::White),
            Cell::new("Next Step").fg(Color::White),
        ]);

    for h in detected {
        let (status, next_step) = if h.installed {
            (
                Cell::new("Installed").fg(Color::Green),
                format!("hm use {} -- --help", h.id),
            )
        } else {
            (
                Cell::new("Not found").fg(Color::DarkGrey),
                format!("hm harness install {}", h.id),
            )
        };
        table.add_row(vec![
            Cell::new(h.id.as_str()),
            Cell::new(alias_label(&h.aliases)),
            Cell::new(h.display_name.as_str()),
            Cell::new(h.target_runtime.as_str()),
            status,
            Cell::new(next_step),
        ]);
    }

    println!("{}", table);

    let installed: Vec<_> = detected.iter().filter(|h| h.installed).collect();
    if !installed.is_empty() {
        use colored::Colorize;
        println!("\n{}", "Details".bold());
        println!("{}", "=".repeat(60));
        for h in &installed {
            println!("\n{}", h.display_name.bold().cyan());
            if let Some(ref bin) = h.binary_path {
                if h.wraps_target_runtime_binary {
                    println!(
                        "  Binary:  {} ({} runtime binary)",
                        bin.display(),
                        h.target_runtime
                    );
                } else {
                    println!("  Binary:  {}", bin.display());
                }
            }
            println!("  Source:  {}", h.package_source);
            println!("  Target:  {}", h.target_runtime);
            if !h.aliases.is_empty() {
                println!("  Aliases: {}", h.aliases.join(", "));
            }
        }
    }

    let found = detected.iter().filter(|h| h.installed).count();
    use colored::Colorize;
    println!(
        "\n{} of {} harness(es) detected",
        found.to_string().bold(),
        detected.len().to_string().bold()
    );
    if found < detected.len() {
        println!(
            "Run {} to install a missing harness, or {} to launch an installed one.",
            "hm harness install <id>".cyan(),
            "hm use <id>".cyan()
        );
    }
}

fn alias_label(aliases: &[String]) -> String {
    if aliases.is_empty() {
        "-".to_string()
    } else {
        aliases.join(", ")
    }
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
