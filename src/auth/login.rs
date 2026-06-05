use anyhow::bail;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::runtimes::manifest::AuthLoginRecord;
use crate::runtimes::registry::RuntimeRegistry;

pub fn run_auth_login(registry: &RuntimeRegistry, runtime: &str) -> anyhow::Result<()> {
    let Some(record) = registry.find(runtime) else {
        println!("Auth login not supported for '{}'.", runtime);
        println!("Check the runtime's documentation for authentication setup.");
        return Ok(());
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
            println!("Auth login not supported for '{}'.", runtime);
            println!("Check the runtime's documentation for authentication setup.");
            Ok(())
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
}
