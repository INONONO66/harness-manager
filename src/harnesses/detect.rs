use std::path::PathBuf;

use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use super::registry::HARNESSES;
use super::types::HarnessSpec;

#[derive(Debug)]
pub struct DetectedHarness {
    pub id: &'static str,
    pub display_name: &'static str,
    pub target_runtime: &'static str,
    pub installed: bool,
    pub binary_path: Option<PathBuf>,
}

pub fn detect_one(spec: &HarnessSpec) -> DetectedHarness {
    let binary = crate::runtimes::find_binary(spec.detect_binaries);
    DetectedHarness {
        id: spec.id,
        display_name: spec.display_name,
        target_runtime: spec.target_runtime,
        installed: binary.is_some(),
        binary_path: binary,
    }
}

pub fn detect_all() -> Vec<DetectedHarness> {
    HARNESSES.iter().map(detect_one).collect()
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
            Cell::new(h.id),
            Cell::new(h.display_name),
            Cell::new(h.target_runtime),
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
            id: "test-sh",
            display_name: "test",
            target_runtime: "Codex CLI",
            package: PackageSpec::Manual { instructions: "" },
            detect_binaries: &["sh"],
            isolation: IsolationSpec {
                subdir: "test",
                spoof_home: false,
                home_subdirs: &[],
                static_envs: &[],
                seed_files: &[],
                caveat: None,
            },
            launch_binary: None,
        };
        let result = detect_one(&spec);
        assert!(result.installed, "sh should be found on PATH");
        assert!(result.binary_path.is_some());
    }

    #[test]
    fn detect_one_missing_binary() {
        let spec = HarnessSpec {
            id: "test-missing",
            display_name: "test",
            target_runtime: "Codex CLI",
            package: PackageSpec::Manual { instructions: "" },
            detect_binaries: &["nonexistent-binary-xyz-99"],
            isolation: IsolationSpec {
                subdir: "test",
                spoof_home: false,
                home_subdirs: &[],
                static_envs: &[],
                seed_files: &[],
                caveat: None,
            },
            launch_binary: None,
        };
        let result = detect_one(&spec);
        assert!(!result.installed);
        assert!(result.binary_path.is_none());
    }

    #[test]
    fn detect_all_returns_five() {
        let results = detect_all();
        assert_eq!(results.len(), 5);
    }
}
