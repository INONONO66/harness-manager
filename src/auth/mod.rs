pub mod login;

use crate::runtimes;
use crate::runtimes::registry::RuntimeRegistry;
use crate::secrets;
use colored::Colorize;

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

pub fn run_auth_status(registry: &RuntimeRegistry) -> anyhow::Result<()> {
    let results = runtimes::detect_all(registry);
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
            Ok(val) => format!("{} ({})", "set".green(), secrets::mask_secret(&val)),
            Err(_) => "unset".dimmed().to_string(),
        };
        println!("  {:30} {}", var, status);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::secrets::mask_secret;

    #[test]
    fn short_env_values_are_never_printed_in_full() {
        let short = "abc123";
        assert_eq!(mask_secret(short), "***");
    }

    #[test]
    fn medium_env_values_show_only_four_char_prefix() {
        let medium = "sk-ant-api-1234567";
        let masked = mask_secret(medium);
        assert!(masked.starts_with("sk-a"));
        assert!(masked.ends_with("***"));
        assert!(
            !masked.contains("1234567"),
            "trailing suffix must not leak: {masked}"
        );
    }
}
