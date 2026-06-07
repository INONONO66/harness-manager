use std::path::PathBuf;

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use super::registry::HarnessRegistry;
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
    let binary = if matches!(&spec.package, PackageSpec::NpmIsolated { .. }) {
        detect_in_isolation_home(spec).or_else(|| crate::runtimes::find_binary(&binary_names))
    } else {
        crate::runtimes::find_binary(&binary_names)
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

fn format_package_source(pkg: &PackageSpec) -> String {
    match pkg {
        PackageSpec::NpmGlobal { package } => format!("npm-global ({package})"),
        PackageSpec::NpmIsolated { package } => format!("npm-isolated ({package})"),
        PackageSpec::NpxInstaller { package, .. } => format!("npx-installer ({package})"),
        PackageSpec::BunxInstaller { package, .. } => format!("bunx-installer ({package})"),
        PackageSpec::PythonTool { package } => format!("python-tool ({package})"),
        PackageSpec::Manual { .. } => "manual".to_string(),
    }
}

fn detect_in_isolation_home(spec: &HarnessSpec) -> Option<PathBuf> {
    let paths = IsolationPaths::try_from_spec(&spec.isolation).ok()?;
    let iso_bin = paths.home.join(".npm").join("bin");
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
mod tests {
    use super::*;
    use crate::harnesses::types::{HarnessSpec, PackageSpec};
    use crate::isolation::spec::IsolationPlan;

    fn test_runtimes() -> crate::runtimes::registry::RuntimeRegistry {
        crate::runtimes::registry::RuntimeRegistry::builtin_only().unwrap()
    }

    fn empty_iso(subdir: &str) -> IsolationPlan {
        IsolationPlan {
            subdir: subdir.to_string(),
            runtime_subdir: subdir.to_string(),
            spoof_home: false,
            home_subdirs: Vec::new(),
            static_envs: Vec::new(),
            seed_files: Vec::new(),
            caveat: None,
        }
    }

    #[test]
    fn detect_one_finds_sh() {
        let spec = HarnessSpec {
            id: "test-sh".to_string(),
            aliases: Vec::new(),
            display_name: "test".to_string(),
            target_runtime: "Codex CLI".to_string(),
            package: PackageSpec::Manual {
                instructions: "".to_string(),
            },
            detect_binaries: vec!["sh".to_string()],
            isolation: empty_iso("test"),
            launch_binary: None,
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);
        assert!(result.installed, "sh should be found on PATH");
        assert!(result.binary_path.is_some());
    }

    #[test]
    fn detect_one_missing_binary() {
        let spec = HarnessSpec {
            id: "test-missing".to_string(),
            aliases: Vec::new(),
            display_name: "test".to_string(),
            target_runtime: "Codex CLI".to_string(),
            package: PackageSpec::Manual {
                instructions: "".to_string(),
            },
            detect_binaries: vec!["nonexistent-binary-xyz-99".to_string()],
            isolation: empty_iso("test"),
            launch_binary: None,
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);
        assert!(!result.installed);
        assert!(result.binary_path.is_none());
    }

    #[test]
    fn detect_all_returns_registered_harnesses() {
        let runtimes = test_runtimes();
        let registry = crate::harnesses::registry::HarnessRegistry::from_sources(
            &[crate::harnesses::registry::HarnessSource::manifest(
                "detect-plugin.toml",
                r#"
schema_version = 1
id = "detect-plugin"
display_name = "Detect Plugin"
target_runtime = "Codex CLI"
detect_binaries = ["nonexistent-detect-plugin-bin"]
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

        let results = detect_all(&registry);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "detect-plugin");
        assert_eq!(results[0].target_runtime, "Codex CLI");
        assert!(!results[0].installed);
    }

    #[test]
    fn detect_all_builtin_only_returns_indexed_builtins() {
        let registry =
            crate::harnesses::registry::HarnessRegistry::builtin_only(&test_runtimes()).unwrap();
        let results = detect_all(&registry);

        assert_eq!(
            results.len(),
            crate::harnesses::builtin::BUILTIN_MANIFESTS.len()
        );
    }

    #[test]
    fn format_package_source_covers_every_kind() {
        assert_eq!(
            format_package_source(&PackageSpec::NpmGlobal {
                package: "p".to_string()
            }),
            "npm-global (p)"
        );
        assert_eq!(
            format_package_source(&PackageSpec::NpmIsolated {
                package: "p".to_string()
            }),
            "npm-isolated (p)"
        );
        assert_eq!(
            format_package_source(&PackageSpec::NpxInstaller {
                package: "p".to_string(),
                args: Vec::new()
            }),
            "npx-installer (p)"
        );
        assert_eq!(
            format_package_source(&PackageSpec::BunxInstaller {
                package: "p".to_string(),
                args: Vec::new()
            }),
            "bunx-installer (p)"
        );
        assert_eq!(
            format_package_source(&PackageSpec::PythonTool {
                package: "p".to_string()
            }),
            "python-tool (p)"
        );
        assert_eq!(
            format_package_source(&PackageSpec::Manual {
                instructions: "x".to_string()
            }),
            "manual"
        );
    }

    #[test]
    fn detect_one_marks_npx_installer_as_wrapping_runtime_binary() {
        let spec = HarnessSpec {
            id: "lazysh".to_string(),
            aliases: Vec::new(),
            display_name: "lazysh".to_string(),
            target_runtime: "shell".to_string(),
            package: PackageSpec::NpxInstaller {
                package: "lazysh-ai".to_string(),
                args: Vec::new(),
            },
            detect_binaries: vec!["sh".to_string()],
            isolation: empty_iso("lazysh"),
            launch_binary: Some("sh".to_string()),
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);
        assert!(result.installed);
        assert!(result.wraps_target_runtime_binary);
        assert_eq!(result.package_source, "npx-installer (lazysh-ai)");
    }

    #[test]
    fn detect_one_does_not_mark_npm_isolated_as_wrapping() {
        let spec = HarnessSpec {
            id: "iso-sh".to_string(),
            aliases: Vec::new(),
            display_name: "iso-sh".to_string(),
            target_runtime: "shell".to_string(),
            package: PackageSpec::NpmIsolated {
                package: "iso-sh-pkg".to_string(),
            },
            detect_binaries: vec!["sh".to_string()],
            isolation: empty_iso("iso-sh"),
            launch_binary: None,
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);
        assert!(!result.wraps_target_runtime_binary);
        assert_eq!(result.package_source, "npm-isolated (iso-sh-pkg)");
    }
}
