use anyhow::bail;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::runtimes::registry::RUNTIMES;
use crate::runtimes::types::{AuthLoginSpec, RuntimeSpec};

pub fn run_auth_login(runtime: &str) -> anyhow::Result<()> {
    let Some(spec) = resolve_runtime(runtime) else {
        println!("Auth login not supported for '{}'.", runtime);
        println!("Check the runtime's documentation for authentication setup.");
        return Ok(());
    };

    match &spec.auth_login {
        AuthLoginSpec::Exec {
            label,
            binary,
            args,
        } => {
            eprintln!("Delegating to {} auth flow...", label);
            let err = Command::new(binary).args(*args).exec();
            bail!("failed to exec {} auth login: {}", binary, err);
        }
        AuthLoginSpec::Message { lines } => {
            for line in *lines {
                println!("{}", line);
            }
            Ok(())
        }
        AuthLoginSpec::Unsupported => {
            println!("Auth login not supported for '{}'.", runtime);
            println!("Check the runtime's documentation for authentication setup.");
            Ok(())
        }
    }
}

fn resolve_runtime(runtime: &str) -> Option<&'static RuntimeSpec> {
    let normalized = normalize_runtime_name(runtime);
    RUNTIMES.iter().find(|spec| {
        normalize_runtime_name(spec.name) == normalized
            || spec
                .binary_names
                .iter()
                .any(|binary| normalize_runtime_name(binary) == normalized)
    })
}

fn normalize_runtime_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_runtime_accepts_binary_and_display_aliases() {
        assert_eq!(resolve_runtime("codex").unwrap().name, "Codex CLI");
        assert_eq!(resolve_runtime("codex-cli").unwrap().name, "Codex CLI");
        assert_eq!(resolve_runtime("claude_code").unwrap().name, "Claude Code");
    }

    #[test]
    fn resolve_runtime_uses_declared_login_spec() {
        let opencode = resolve_runtime("opencode").unwrap();

        assert!(matches!(opencode.auth_login, AuthLoginSpec::Message { .. }));
    }
}
