use std::collections::HashMap;

use super::spec::IsolationRecipe;
use super::{subst_tokens, IsolationPaths};

pub const GLOBAL_AI_STRIP: &[&str] = &[
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
    "OPENAI_ORG_ID",
    "OPENAI_PROJECT_ID",
    "CODEX_HOME",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_TMPDIR",
    "CLAUDE_CODE_DEBUG_LOGS_DIR",
    "OPENCODE_CONFIG_DIR",
    "PI_CODING_AGENT_DIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
];

// RedirectOnly: credential-only strip list (no config-path vars).
// Used by build_redirect_only_env to strip AI API keys while preserving
// host environment variables like HOME, CARGO_HOME, GITHUB_TOKEN, PATH.
pub const REDIRECT_ONLY_CREDENTIAL_STRIP: &[&str] = &[
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
    "OPENAI_ORG_ID",
    "OPENAI_PROJECT_ID",
];

const SAFE_INHERITED_ENV: &[&str] = &[
    "PATH",
    "LANG",
    "TERM",
    "COLORTERM",
    "NO_COLOR",
    "FORCE_COLOR",
    "TMPDIR",
    "TEMP",
    "TMP",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "SystemRoot",
    "COMSPEC",
    "PATHEXT",
    "SSH_AUTH_SOCK",
    "GH_CONFIG_DIR",
    "GIT_CONFIG_GLOBAL",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "BUN_INSTALL",
    "NPM_CONFIG_USERCONFIG",
];

fn is_safe_inherited_env_key(key: &str) -> bool {
    SAFE_INHERITED_ENV.contains(&key) || key.starts_with("LC_")
}

pub fn build_isolation_env(
    spec: &(impl IsolationRecipe + ?Sized),
    paths: &IsolationPaths,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (k, v_template) in spec.static_envs() {
        out.insert(k.to_string(), subst_tokens(v_template, paths));
    }
    out
}

pub fn build_sanitized_isolation_env(
    inherited: &HashMap<String, String>,
    spec: &(impl IsolationRecipe + ?Sized),
    paths: &IsolationPaths,
) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = inherited
        .iter()
        .filter(|(key, _)| is_safe_inherited_env_key(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    for var in GLOBAL_AI_STRIP {
        out.remove(*var);
    }
    if let Some(path) = out.get("PATH").cloned() {
        let filtered: Vec<&str> = path
            .split(':')
            .filter(|dir| !dir.contains("mise/shims") && !dir.contains("asdf/shims"))
            .collect();
        out.insert("PATH".to_string(), filtered.join(":"));
    }
    // Derive CLI config paths from HOME or XDG_CONFIG_HOME if not explicitly set.
    if !out.contains_key("GH_CONFIG_DIR") {
        if let Some(xdg_config) = inherited.get("XDG_CONFIG_HOME") {
            out.insert("GH_CONFIG_DIR".to_string(), format!("{}/gh", xdg_config));
        } else if let Some(home) = inherited.get("HOME") {
            out.insert("GH_CONFIG_DIR".to_string(), format!("{}/.config/gh", home));
        }
    }
    if !out.contains_key("GIT_CONFIG_GLOBAL") {
        if let Some(home) = inherited.get("HOME") {
            out.insert(
                "GIT_CONFIG_GLOBAL".to_string(),
                format!("{}/.gitconfig", home),
            );
        }
    }
    if !out.contains_key("CARGO_HOME") {
        if let Some(home) = inherited.get("HOME") {
            out.insert("CARGO_HOME".to_string(), format!("{}/.cargo", home));
        }
    }
    if !out.contains_key("RUSTUP_HOME") {
        if let Some(home) = inherited.get("HOME") {
            out.insert("RUSTUP_HOME".to_string(), format!("{}/.rustup", home));
        }
    }
    if !out.contains_key("BUN_INSTALL") {
        if let Some(home) = inherited.get("HOME") {
            out.insert("BUN_INSTALL".to_string(), format!("{}/.bun", home));
        }
    }
    if !out.contains_key("NPM_CONFIG_USERCONFIG") {
        if let Some(home) = inherited.get("HOME") {
            out.insert(
                "NPM_CONFIG_USERCONFIG".to_string(),
                format!("{}/.npmrc", home),
            );
        }
    }
    for (k, v) in build_isolation_env(spec, paths) {
        out.insert(k, v);
    }
    // This is THE SpoofHome builder: HOME is always redirected to the isolated
    // tree. Inserted after the static_envs overlay so the spoof always wins.
    out.insert("HOME".to_string(), paths.home.to_string_lossy().to_string());
    out
}

// RedirectOnly: passes host env through like plain runtime usage.
// Only AI credentials are stripped. GITHUB_TOKEN and other host secrets
// are visible to the child process — same as running the runtime directly.
// SpoofHome mode (build_sanitized_isolation_env) provides stronger isolation.
pub fn build_redirect_only_env(
    inherited: &HashMap<String, String>,
    spec: &(impl IsolationRecipe + ?Sized),
    paths: &IsolationPaths,
) -> HashMap<String, String> {
    let mut out = inherited.clone();
    for var in REDIRECT_ONLY_CREDENTIAL_STRIP {
        out.remove(*var);
    }
    for (k, v) in build_isolation_env(spec, paths) {
        out.insert(k, v);
    }
    out
}
