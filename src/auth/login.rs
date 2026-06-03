use std::os::unix::process::CommandExt;
use std::process::Command;
use anyhow::bail;

pub fn run_auth_login(runtime: &str) -> anyhow::Result<()> {
    let normalized = runtime.to_lowercase();

    match normalized.as_str() {
        "claude" | "claude-code" | "claude_code" => {
            eprintln!("Delegating to Claude Code auth flow...");
            let err = Command::new("claude").exec();
            bail!("failed to exec claude: {}", err);
        }
        "codex" | "codex-cli" | "codex_cli" => {
            eprintln!("Delegating to Codex auth flow...");
            let err = Command::new("codex").arg("auth").arg("login").exec();
            bail!("failed to exec codex auth login: {}", err);
        }
        "opencode" => {
            println!("OpenCode uses provider-specific authentication.");
            println!("Set API keys via environment variables:");
            println!("  export ANTHROPIC_API_KEY=sk-...");
            println!("  export OPENAI_API_KEY=sk-...");
            println!("Or run `opencode` to authenticate interactively.");
            Ok(())
        }
        "gemini" | "gemini-cli" => {
            eprintln!("Delegating to Gemini CLI auth flow...");
            let err = Command::new("gemini").arg("auth").arg("login").exec();
            bail!("failed to exec gemini auth login: {}", err);
        }
        _ => {
            println!("Auth login not supported for '{}'.", runtime);
            println!("Check the runtime's documentation for authentication setup.");
            Ok(())
        }
    }
}
