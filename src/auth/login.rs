use anyhow::bail;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::runtimes::manifest::AuthLoginRecord;
use crate::runtimes::registry::RuntimeRegistry;

pub fn run_auth_login(registry: &RuntimeRegistry, runtime: &str) -> anyhow::Result<()> {
    let Some(record) = registry.find(runtime) else {
        let known: Vec<String> = registry
            .records()
            .iter()
            .filter_map(|r| r.binary_names.first().cloned())
            .collect();
        bail!(
            "unknown runtime '{}'. Known runtimes: {}. Run `hm detect` for status.",
            runtime,
            if known.is_empty() {
                "(none)".to_string()
            } else {
                known.join(", ")
            }
        );
    };

    match &record.auth_login {
        AuthLoginRecord::Exec {
            label,
            binary,
            args,
        } => {
            eprintln!("Delegating to {} auth flow...", label);
            let err = Command::new(binary).args(args).exec();
            bail!("failed to exec {} auth login: {}", binary, err);
        }
        AuthLoginRecord::Message { lines } => {
            for line in lines {
                println!("{}", line);
            }
            Ok(())
        }
        AuthLoginRecord::Unsupported => {
            bail!(
                "auth login is not supported for runtime '{}'. Check the runtime's documentation for authentication setup.",
                runtime
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_runtime_accepts_binary_and_display_aliases() {
        let registry = RuntimeRegistry::builtin_only().unwrap();
        assert_eq!(registry.find("codex").unwrap().name, "Codex CLI");
        assert_eq!(registry.find("codex-cli").unwrap().name, "Codex CLI");
        assert_eq!(registry.find("claude_code").unwrap().name, "Claude Code");
    }

    #[test]
    fn opencode_uses_declared_message_login() {
        let registry = RuntimeRegistry::builtin_only().unwrap();
        let opencode = registry.find("opencode").unwrap();
        assert!(matches!(
            opencode.auth_login,
            AuthLoginRecord::Message { .. }
        ));
    }

    #[test]
    fn unknown_runtime_returns_error_so_scripts_can_detect_failure() {
        let registry = RuntimeRegistry::builtin_only().unwrap();
        let err = run_auth_login(&registry, "no-such-runtime-xyz").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown runtime"),
            "expected 'unknown runtime' in error, got: {msg}"
        );
        assert!(
            msg.contains("no-such-runtime-xyz"),
            "error must mention the bad name, got: {msg}"
        );
    }
}
