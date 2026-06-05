use std::path::PathBuf;

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use super::registry::HarnessRegistry;
use super::types::HarnessSpec;

#[derive(Debug)]
pub struct DetectedHarness {
    pub id: String,
    pub display_name: String,
    pub target_runtime: String,
    pub installed: bool,
    pub binary_path: Option<PathBuf>,
}

pub fn detect_one(spec: &HarnessSpec) -> DetectedHarness {
    let binary_names: Vec<&str> = spec.detect_binaries.iter().map(String::as_str).collect();
    let binary = crate::runtimes::find_binary(&binary_names);
    DetectedHarness {
        id: spec.id.clone(),
        display_name: spec.display_name.clone(),
        target_runtime: spec.target_runtime.clone(),
        installed: binary.is_some(),
        binary_path: binary,
    }
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
            Cell::new("Display Name").fg(Color::White),
            Cell::new("Target Runtime").fg(Color::White),
            Cell::new("Status").fg(Color::White),
        ]);

    for h in detected {
        let status = if h.installed {
            Cell::new("Installed").fg(Color::Green)
        } else {
            Cell::new("Not found").fg(Color::DarkGrey)
        };
        table.add_row(vec![
            Cell::new(h.id.as_str()),
            Cell::new(h.display_name.as_str()),
            Cell::new(h.target_runtime.as_str()),
            status,
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
                println!("  Binary:  {}", bin.display());
            }
            println!("  Target:  {}", h.target_runtime);
        }
    }

    let found = detected.iter().filter(|h| h.installed).count();
    use colored::Colorize;
    println!("\n{} harness(es) detected", found.to_string().bold());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harnesses::types::{HarnessSpec, PackageSpec};
    use crate::runtimes::types::IsolationSpec;

    #[test]
    fn detect_one_finds_sh() {
        let spec = HarnessSpec {
            id: "test-sh".to_string(),
            display_name: "test".to_string(),
            target_runtime: "Codex CLI".to_string(),
            package: PackageSpec::Manual {
                instructions: "".to_string(),
            },
            detect_binaries: vec!["sh".to_string()],
            isolation: crate::isolation::spec::IsolationPlan::from_runtime(&IsolationSpec {
                subdir: "test",
                spoof_home: false,
                home_subdirs: &[],
                static_envs: &[],
                seed_files: &[],
                caveat: None,
            }),
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
            display_name: "test".to_string(),
            target_runtime: "Codex CLI".to_string(),
            package: PackageSpec::Manual {
                instructions: "".to_string(),
            },
            detect_binaries: vec!["nonexistent-binary-xyz-99".to_string()],
            isolation: crate::isolation::spec::IsolationPlan::from_runtime(&IsolationSpec {
                subdir: "test",
                spoof_home: false,
                home_subdirs: &[],
                static_envs: &[],
                seed_files: &[],
                caveat: None,
            }),
            launch_binary: None,
            launch_args: Vec::new(),
        };
        let result = detect_one(&spec);
        assert!(!result.installed);
        assert!(result.binary_path.is_none());
    }

    #[test]
    fn detect_all_returns_registered_harnesses() {
        let registry = crate::harnesses::registry::HarnessRegistry::from_sources(&[
            crate::harnesses::registry::HarnessSource::manifest(
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
            ),
        ])
        .unwrap();

        let results = detect_all(&registry);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "detect-plugin");
        assert_eq!(results[0].target_runtime, "Codex CLI");
        assert!(!results[0].installed);
    }

    #[test]
    fn detect_all_builtin_only_returns_indexed_builtins() {
        let registry = crate::harnesses::registry::HarnessRegistry::builtin_only().unwrap();
        let results = detect_all(&registry);

        assert_eq!(
            results.len(),
            crate::harnesses::builtin::BUILTIN_MANIFESTS.len()
        );
    }
}
