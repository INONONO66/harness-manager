use colored::Colorize;
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use crate::runtimes;
use crate::runtimes::registry::RuntimeRegistry;
use crate::runtimes::types::AuthStatus;

fn auth_summary(sources: &[AuthStatus]) -> (String, Color) {
    if sources.is_empty() {
        return ("Not configured".to_string(), Color::Red);
    }

    let mut parts = Vec::new();
    let mut worst_color = Color::Green;

    for s in sources {
        match s {
            AuthStatus::Valid { detail } => parts.push(detail.clone()),
            AuthStatus::ExpiresSoon { detail } => {
                parts.push(detail.clone());
                worst_color = Color::Yellow;
            }
            AuthStatus::Expired { detail } => {
                parts.push(detail.clone());
                worst_color = Color::Red;
            }
            _ => {}
        }
    }

    if parts.is_empty() {
        ("Not configured".to_string(), Color::Red)
    } else {
        (parts.join(" + "), worst_color)
    }
}

fn auth_detail_lines(sources: &[AuthStatus]) -> Vec<String> {
    if sources.is_empty() {
        return vec![format!("{} Not configured", "\u{274c}")];
    }
    sources
        .iter()
        .map(|s| format!("{} {}", s.status_icon(), s.status_text()))
        .collect()
}

fn runtime_command(registry: &RuntimeRegistry, name: &str) -> String {
    registry
        .find_by_display_name(name)
        .and_then(|record| record.binary_names.first().cloned())
        .unwrap_or_else(|| "-".to_string())
}

fn has_configured_auth(sources: &[AuthStatus]) -> bool {
    sources.iter().any(|source| {
        matches!(
            source,
            AuthStatus::Valid { .. } | AuthStatus::ExpiresSoon { .. }
        )
    })
}

fn next_step(
    registry: &RuntimeRegistry,
    name: &str,
    installed: bool,
    auth_sources: &[AuthStatus],
) -> String {
    let command = runtime_command(registry, name);
    if !installed {
        return format!("Install {}", name);
    }
    if has_configured_auth(auth_sources) {
        format!("hm use {} -- --help", command)
    } else {
        format!("hm auth login {}", command)
    }
}

pub fn run_detect(registry: &RuntimeRegistry) -> anyhow::Result<()> {
    let results = runtimes::detect_all(registry);

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Runtime").fg(Color::White),
            Cell::new("Command").fg(Color::White),
            Cell::new("Status").fg(Color::White),
            Cell::new("Version").fg(Color::White),
            Cell::new("Auth").fg(Color::White),
            Cell::new("Next Step").fg(Color::White),
        ]);

    for rt in &results {
        let status = if rt.installed {
            Cell::new("Installed").fg(Color::Green)
        } else {
            Cell::new("Not found").fg(Color::DarkGrey)
        };

        let version = Cell::new(rt.version.as_deref().unwrap_or("-"));

        let (auth_text, auth_color) = auth_summary(&rt.auth_sources);
        let auth = Cell::new(&auth_text).fg(auth_color);

        table.add_row(vec![
            Cell::new(&rt.name),
            Cell::new(runtime_command(registry, &rt.name)),
            status,
            version,
            auth,
            Cell::new(next_step(
                registry,
                &rt.name,
                rt.installed,
                &rt.auth_sources,
            )),
        ]);
    }

    println!("{}", table);

    let installed: Vec<_> = results.iter().filter(|r| r.installed).collect();

    if !installed.is_empty() {
        println!("\n{}", "Details".bold());
        println!("{}", "=".repeat(60));

        for rt in &installed {
            println!("\n{}", rt.name.bold().cyan());

            if let Some(ref bin) = rt.binary_path {
                println!("  Binary:  {}", bin.display());
            }
            if let Some(ref cfg) = rt.config_path {
                println!("  Config:  {}", cfg.display());
            }
            println!("  Command: hm use {}", runtime_command(registry, &rt.name));

            let lines = auth_detail_lines(&rt.auth_sources);
            for (i, line) in lines.iter().enumerate() {
                if i == 0 {
                    println!("  Auth:    {}", line);
                } else {
                    println!("           {}", line);
                }
            }
        }
    }

    let found = installed.len();
    println!("\n{} runtime(s) detected", found.to_string().bold());

    if found == 0 {
        println!(
            "{}",
            "No agent runtimes found. Install Claude Code, Codex CLI, Gajae-Code, Grok CLI, OpenCode, or Pi.".yellow()
        );
    } else {
        println!(
            "Run {} to launch a configured runtime, or {} to configure auth.",
            "hm use <command>".cyan(),
            "hm auth login <command>".cyan()
        );
    }

    Ok(())
}
