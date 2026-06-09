use std::path::{Path, PathBuf};

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
    let binary = match &spec.package {
        PackageSpec::NpmIsolated { .. } => detect_in_isolation_home(spec, ".npm/bin")
            .or_else(|| crate::runtimes::find_binary(&binary_names)),
        PackageSpec::PythonTool { .. } => detect_in_isolation_home(spec, ".local/bin")
            .or_else(|| crate::runtimes::find_binary(&binary_names)),
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
        PackageSpec::NpmGlobal { package } => format!("npm-global ({package})"),
        PackageSpec::NpmIsolated { package } => format!("npm-isolated ({package})"),
        PackageSpec::NpxInstaller { package, .. } => format!("npx-installer ({package})"),
        PackageSpec::BunxInstaller { package, .. } => format!("bunx-installer ({package})"),
        PackageSpec::PythonTool { package } => format!("python-tool ({package})"),
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

    fn unique_subdir(label: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("hm-detect-test-{}-{}-{nanos}", label, std::process::id())
    }

    #[test]
    fn detect_one_marks_npx_installer_as_wrapping_runtime_binary() {
        let subdir = unique_subdir("wrapping-runtime");
        let isolation = empty_iso(&subdir);
        let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
        let cache_pkg = paths
            .home
            .join(".npm")
            .join("_npx")
            .join("seedhash")
            .join("node_modules")
            .join("lazysh-ai");
        std::fs::create_dir_all(&cache_pkg).expect("seed npx cache");

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
            isolation,
            launch_binary: Some("sh".to_string()),
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);

        assert!(
            result.installed,
            "NpxInstaller with cached package must report installed"
        );
        assert!(result.wraps_target_runtime_binary);
        assert_eq!(result.package_source, "npx-installer (lazysh-ai)");

        let _ = std::fs::remove_dir_all(&paths.base);
    }

    #[test]
    fn detect_one_npx_installer_not_installed_when_cache_empty_even_with_runtime_on_path() {
        // Regression for issue #5: PATH-only lookup falsely reported Installed
        // for NpxInstaller harnesses when only the target runtime binary was
        // on host PATH after `hm harness remove --purge`.
        let subdir = unique_subdir("npx-no-cache");
        let isolation = empty_iso(&subdir);

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
            isolation: isolation.clone(),
            launch_binary: Some("sh".to_string()),
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);

        assert!(
            !result.installed,
            "NpxInstaller without npx cache must NOT report installed even when runtime binary is on PATH"
        );
        assert!(
            result.binary_path.is_none(),
            "NpxInstaller without npx cache must report no binary path"
        );

        let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
        let _ = std::fs::remove_dir_all(&paths.base);
    }

    #[test]
    fn detect_one_bunx_installer_not_installed_when_cache_empty_even_with_runtime_on_path() {
        // Regression for issue #5 (BunxInstaller variant).
        let subdir = unique_subdir("bunx-no-cache");
        let isolation = empty_iso(&subdir);

        let spec = HarnessSpec {
            id: "bunsh".to_string(),
            aliases: Vec::new(),
            display_name: "bunsh".to_string(),
            target_runtime: "shell".to_string(),
            package: PackageSpec::BunxInstaller {
                package: "oh-my-bunsh".to_string(),
                args: Vec::new(),
            },
            detect_binaries: vec!["sh".to_string()],
            isolation: isolation.clone(),
            launch_binary: Some("sh".to_string()),
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);

        assert!(
            !result.installed,
            "BunxInstaller without bun cache must NOT report installed even when runtime binary is on PATH"
        );
        assert!(result.binary_path.is_none());

        let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
        let _ = std::fs::remove_dir_all(&paths.base);
    }

    #[test]
    fn detect_one_bunx_installer_installed_when_cache_present() {
        let subdir = unique_subdir("bunx-cache");
        let isolation = empty_iso(&subdir);
        let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
        let cache_root = paths.home.join(".bun").join("install").join("cache");
        let pkg_versioned = cache_root.join("oh-my-bunsh@1.2.3");
        std::fs::create_dir_all(&pkg_versioned).expect("seed bunx cache");

        let spec = HarnessSpec {
            id: "bunsh".to_string(),
            aliases: Vec::new(),
            display_name: "bunsh".to_string(),
            target_runtime: "shell".to_string(),
            package: PackageSpec::BunxInstaller {
                package: "oh-my-bunsh".to_string(),
                args: Vec::new(),
            },
            detect_binaries: vec!["sh".to_string()],
            isolation,
            launch_binary: Some("sh".to_string()),
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);

        assert!(
            result.installed,
            "BunxInstaller with versioned cache dir must report installed"
        );
        assert_eq!(result.package_source, "bunx-installer (oh-my-bunsh)");

        let _ = std::fs::remove_dir_all(&paths.base);
    }

    #[test]
    fn detect_one_python_tool_installed_from_isolated_local_bin() {
        let subdir = unique_subdir("python-tool-bin");
        let isolation = empty_iso(&subdir);
        let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
        let bin_dir = paths.home.join(".local").join("bin");
        let binary = bin_dir.join("python-tool-bin");
        std::fs::create_dir_all(&bin_dir).expect("seed python tool bin dir");
        std::fs::write(&binary, "#!/bin/sh\n").expect("seed python tool binary");

        let spec = HarnessSpec {
            id: "python-tool-harness".to_string(),
            aliases: Vec::new(),
            display_name: "python-tool-harness".to_string(),
            target_runtime: "shell".to_string(),
            package: PackageSpec::PythonTool {
                package: "python-tool-package".to_string(),
            },
            detect_binaries: vec!["python-tool-bin".to_string()],
            isolation,
            launch_binary: Some("python-tool-bin".to_string()),
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);

        assert!(
            result.installed,
            "PythonTool must report installed when pipx/uv created a binary under isolated HOME/.local/bin"
        );
        assert_eq!(result.binary_path, Some(binary));

        let _ = std::fs::remove_dir_all(&paths.base);
    }

    #[test]
    fn package_cache_installed_at_finds_npx_node_modules_one_hash_deep() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = "test-only-npx-pkg";
        let leaf = tmp
            .path()
            .join(".npm")
            .join("_npx")
            .join("abcdef0123")
            .join("node_modules")
            .join(pkg);
        std::fs::create_dir_all(&leaf).unwrap();

        let spec = PackageSpec::NpxInstaller {
            package: pkg.to_string(),
            args: Vec::new(),
        };
        let result = package_cache_installed_at(tmp.path(), &spec);

        let path = result.expect("npx cache lookup finds seeded package");
        assert!(path.ends_with(format!("node_modules/{pkg}")));
    }

    #[test]
    fn package_cache_installed_at_returns_none_when_no_npx_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = PackageSpec::NpxInstaller {
            package: "ghost-pkg".to_string(),
            args: Vec::new(),
        };
        assert!(package_cache_installed_at(tmp.path(), &spec).is_none());
    }

    #[test]
    fn package_cache_installed_at_finds_bunx_cache_versioned_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = "test-only-bunx-pkg";
        let entry = tmp
            .path()
            .join(".bun")
            .join("install")
            .join("cache")
            .join(format!("{pkg}@2.0.1"));
        std::fs::create_dir_all(&entry).unwrap();

        let spec = PackageSpec::BunxInstaller {
            package: pkg.to_string(),
            args: Vec::new(),
        };
        let result = package_cache_installed_at(tmp.path(), &spec);
        assert!(result
            .expect("bunx cache lookup hit")
            .ends_with(format!(".bun/install/cache/{pkg}@2.0.1")));
    }

    #[test]
    fn package_cache_installed_at_finds_bunx_cache_bare_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = "test-only-bunx-pkg";
        let bare = tmp
            .path()
            .join(".bun")
            .join("install")
            .join("cache")
            .join(pkg);
        std::fs::create_dir_all(&bare).unwrap();

        let spec = PackageSpec::BunxInstaller {
            package: pkg.to_string(),
            args: Vec::new(),
        };
        let result = package_cache_installed_at(tmp.path(), &spec);
        assert!(result.is_some());
    }

    #[test]
    fn package_cache_installed_at_finds_bunx_xdg_cache_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg = "test-only-bunx-pkg";
        let entry = tmp
            .path()
            .join(".cache")
            .join(".bun")
            .join("install")
            .join("cache")
            .join(format!("{pkg}@2.0.1@@@1"));
        std::fs::create_dir_all(&entry).unwrap();

        let spec = PackageSpec::BunxInstaller {
            package: pkg.to_string(),
            args: Vec::new(),
        };
        let result = package_cache_installed_at(tmp.path(), &spec);

        assert!(result
            .expect("bunx XDG cache lookup hit")
            .ends_with(format!(".cache/.bun/install/cache/{pkg}@2.0.1@@@1")));
    }

    #[test]
    fn package_cache_installed_at_ignores_unrelated_bunx_cache_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let unrelated = tmp
            .path()
            .join(".bun")
            .join("install")
            .join("cache")
            .join("some-other-package@1.0.0");
        std::fs::create_dir_all(&unrelated).unwrap();

        let spec = PackageSpec::BunxInstaller {
            package: "test-only-bunx-pkg".to_string(),
            args: Vec::new(),
        };
        assert!(package_cache_installed_at(tmp.path(), &spec).is_none());
    }

    #[test]
    fn package_cache_installed_at_returns_none_for_non_installer_kinds() {
        let tmp = tempfile::tempdir().unwrap();
        for spec in [
            PackageSpec::NpmGlobal {
                package: "p".to_string(),
            },
            PackageSpec::NpmIsolated {
                package: "p".to_string(),
            },
            PackageSpec::PythonTool {
                package: "p".to_string(),
            },
            PackageSpec::Manual {
                instructions: "x".to_string(),
            },
        ] {
            assert!(
                package_cache_installed_at(tmp.path(), &spec).is_none(),
                "{:?} must not match installer cache",
                spec
            );
        }
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
