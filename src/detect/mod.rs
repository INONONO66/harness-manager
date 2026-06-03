use colored::Colorize;
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};

use crate::runtimes;
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

pub fn run_detect() -> anyhow::Result<()> {
    let results = runtimes::detect_all();

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Runtime").fg(Color::White),
            Cell::new("Status").fg(Color::White),
            Cell::new("Version").fg(Color::White),
            Cell::new("Auth").fg(Color::White),
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

        table.add_row(vec![Cell::new(&rt.name), status, version, auth]);
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
            "No agent runtimes found. Install Claude Code, Codex CLI, OpenCode, or Pi.".yellow()
        );
    }

    Ok(())
}
