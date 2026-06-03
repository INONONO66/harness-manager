pub mod login;

use colored::Colorize;
use crate::runtimes;

const ENV_VARS_TO_CHECK: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "CODEX_API_KEY",
    "CODEX_ACCESS_TOKEN",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
    "GROQ_API_KEY",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
];

pub fn run_auth_status() -> anyhow::Result<()> {
    let results = runtimes::detect_all();
    let installed: Vec<_> = results.iter().filter(|r| r.installed).collect();

    if installed.is_empty() {
        println!("{}", "No agent runtimes installed.".yellow());
        return Ok(());
    }

    for rt in &installed {
        println!("{}", rt.name.bold().cyan());
        println!("{}", "-".repeat(50));

        if let Some(ref bin) = rt.binary_path {
            println!("  Binary:   {}", bin.display());
        }
        if let Some(ref cfg) = rt.config_path {
            println!("  Config:   {}", cfg.display());
        }

        if rt.auth_sources.is_empty() {
            println!("  Auth:     {}", "Not configured".red());
        } else {
            for (i, src) in rt.auth_sources.iter().enumerate() {
                let prefix = if i == 0 { "  Auth:    " } else { "           " };
                println!("{} {}", prefix, src);
            }
        }

        println!();
    }

    println!("{}", "Environment Variables".bold());
    println!("{}", "-".repeat(50));
    for var in ENV_VARS_TO_CHECK {
        let status = match std::env::var(var) {
            Ok(val) => {
                let display = if val.len() > 12 {
                    format!("{}...{}", &val[..4], &val[val.len()-4..])
                } else {
                    val
                };
                format!("{} ({})", "set".green(), display)
            }
            Err(_) => "unset".dimmed().to_string(),
        };
        println!("  {:30} {}", var, status);
    }

    Ok(())
}
