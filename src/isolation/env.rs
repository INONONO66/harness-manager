use std::collections::HashMap;

use crate::runtimes::types::IsolationSpec;

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
    "OMX_ROOT",
    "OMX_STATE_ROOT",
    "OMX_TEAM_STATE_ROOT",
    "OUROBOROS_CODEX_HOME",
    "OUROBOROS_FAKE_HOME",
    "OUROBOROS_OPENCODE_CONFIG_DIR",
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
];

fn is_safe_inherited_env_key(key: &str) -> bool {
    SAFE_INHERITED_ENV.contains(&key) || key.starts_with("LC_")
}

pub fn build_isolation_env(
    spec: &IsolationSpec,
    paths: &IsolationPaths,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if spec.spoof_home {
        out.insert("HOME".to_string(), paths.home.to_string_lossy().to_string());
    }
    for (k, v_template) in spec.static_envs {
        out.insert(k.to_string(), subst_tokens(v_template, paths));
    }
    out
}

pub fn build_sanitized_isolation_env(
    inherited: &HashMap<String, String>,
    spec: &IsolationSpec,
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
    for (k, v) in build_isolation_env(spec, paths) {
        out.insert(k, v);
    }
    out
}
