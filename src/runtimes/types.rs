use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DetectedRuntime {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub binary_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub auth_sources: Vec<AuthStatus>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AuthStatus {
    Valid { detail: String },
    ExpiresSoon { detail: String },
    Expired { detail: String },
    NotConfigured,
    Unknown,
}

impl AuthStatus {
    pub fn status_icon(&self) -> &str {
        match self {
            Self::Valid { .. } => "\u{2705}",
            Self::ExpiresSoon { .. } => "\u{26a0}\u{fe0f}",
            Self::Expired { .. } => "\u{274c}",
            Self::NotConfigured => "\u{274c}",
            Self::Unknown => "?",
        }
    }

    pub fn status_text(&self) -> String {
        match self {
            Self::Valid { detail } => format!("Valid ({})", detail),
            Self::ExpiresSoon { detail } => format!("Expires soon ({})", detail),
            Self::Expired { detail } => format!("Expired ({})", detail),
            Self::NotConfigured => "Not configured".to_string(),
            Self::Unknown => "Unknown".to_string(),
        }
    }
}

impl fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status_icon(), self.status_text())
    }
}

// ---------------------------------------------------------------------------
// Runtime spec: declarative definition of how to detect a runtime
// ---------------------------------------------------------------------------

/// How to find the config directory for a runtime.
#[derive(Debug, Clone)]
pub enum ConfigLocator {
    /// Check env var first, then fall back to a path relative to $HOME.
    EnvOrHome {
        env_var: &'static str,
        home_relative: &'static str,
    },
    /// Path relative to XDG config dir (e.g. "opencode" → ~/.config/opencode).
    XdgConfig {
        subdir: &'static str,
        env_override: &'static str,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AuthProbe {
    EnvKeys {
        vars: &'static [&'static str],
        label: &'static str,
    },
    JsonFile {
        relative_path: &'static str,
        existence_field: &'static str,
        label: &'static str,
    },
    OAuthFile {
        relative_path: &'static str,
        token_field: &'static str,
        label: &'static str,
    },
    /// OAuth file with nested path (e.g. "claudeAiOauth.accessToken").
    NestedOAuthFile {
        relative_path: &'static str,
        path: &'static [&'static str],
        label: &'static str,
    },
    /// Non-empty JSON file in a data directory (resolved separately from config dir).
    DataDirJsonFile {
        data_subdir: &'static str,
        file_name: &'static str,
        label: &'static str,
    },
    KeychainHeuristic {
        marker_file: &'static str,
        label: &'static str,
    },
}

/// Declarative spec for one agent runtime.
#[derive(Debug, Clone)]
pub struct RuntimeSpec {
    pub name: &'static str,
    /// Binary names to search in PATH (first found wins).
    pub binary_names: &'static [&'static str],
    /// Arg to pass to the binary to get version output.
    pub version_arg: &'static str,
    /// How to locate the config directory.
    pub config_locator: ConfigLocator,
    /// Config file names to look for inside the config dir (first found wins).
    pub config_files: &'static [&'static str],
    pub auth_probes: &'static [AuthProbe],
    pub injection: Option<&'static InjectionSpec>,
    /// Per-runtime isolation recipe for `hm use`. None = no isolation (e.g. Claude in Phase 1).
    pub isolation: Option<&'static IsolationSpec>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct InjectionSpec {
    pub endpoint_env: &'static str,
    pub api_key_env: &'static str,
    pub proxy_envs: &'static [&'static str],
    pub strip_envs: &'static [&'static str],
    /// SDK appends /v1 automatically — strip trailing /v1 from endpoint before injecting.
    pub endpoint_strip_v1: bool,
}

// ---------------------------------------------------------------------------
// Isolation spec: declarative recipe for per-runtime environment redirection
// ---------------------------------------------------------------------------

/// Per-runtime isolation: build $HM tree, redirect env, optionally seed config files.
///
/// Applied by `launch::run_use` when `RuntimeSpec.isolation` is Some.
/// Token substitution in `static_envs` values and `seed_files` (both path and content):
///   `{home}`  → `$HM/runtimes/<subdir>/home`
///   `{state}` → `$HM/runtimes/<subdir>/state`
///   `{tmp}`   → `$HM/runtimes/<subdir>/tmp`
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IsolationSpec {
    /// Subdirectory under `$HM/runtimes/` for this runtime (e.g. "codex").
    pub subdir: &'static str,
    /// If true, set `HOME=$HM/runtimes/<subdir>/home` (Layer 2 — catches hardcoded ~/.X paths).
    pub spoof_home: bool,
    /// Directories to pre-create under `home/` (e.g. `[".codex"]` or `[".config/opencode"]`).
    /// Ensures the runtime finds expected layout when it writes to its env-redirected location.
    pub home_subdirs: &'static [&'static str],
    /// Environment variables to set (key, value-template). Value is token-substituted.
    pub static_envs: &'static [(&'static str, &'static str)],
    /// Config files to seed (path-template, content-template). Both are token-substituted.
    /// Policy: create-if-missing (Phase 1). User edits are preserved across launches.
    pub seed_files: &'static [(&'static str, &'static str)],
    /// Optional user-facing warning printed to stderr at launch (e.g. Claude Keychain note).
    pub caveat: Option<&'static str>,
}
