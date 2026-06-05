use super::types::*;

mod injection;
mod isolation;

pub use isolation::CLAUDE_KEYCHAIN_ISOLATION;

use injection::{CLAUDE_INJECTION, CODEX_INJECTION, OPENCODE_INJECTION};
use isolation::{CLAUDE_ISOLATION, CODEX_ISOLATION, OPENCODE_ISOLATION, PI_ISOLATION};

pub static RUNTIMES: &[RuntimeSpec] = &[
    RuntimeSpec {
        name: "Claude Code",
        binary_names: &["claude"],
        version_arg: "--version",
        config_locator: ConfigLocator::EnvOrHome {
            env_var: "CLAUDE_CONFIG_DIR",
            home_relative: ".claude",
        },
        config_files: &["settings.json"],
        auth_probes: &[
            AuthProbe::NestedOAuthFile {
                relative_path: ".credentials.json",
                path: &["claudeAiOauth", "accessToken"],
                label: "OAuth",
            },
            AuthProbe::KeychainHeuristic {
                marker_file: "settings.json",
                label: "OAuth (macOS Keychain)",
            },
            AuthProbe::EnvKeys {
                vars: &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"],
                label: "API key",
            },
        ],
        injection: Some(&CLAUDE_INJECTION),
        isolation: Some(&CLAUDE_ISOLATION),
        keychain_isolation: Some(&CLAUDE_KEYCHAIN_ISOLATION),
        auth_login: AuthLoginSpec::Exec {
            label: "Claude Code",
            binary: "claude",
            args: &[],
        },
    },
    RuntimeSpec {
        name: "Codex CLI",
        binary_names: &["codex"],
        version_arg: "--version",
        config_locator: ConfigLocator::EnvOrHome {
            env_var: "CODEX_HOME",
            home_relative: ".codex",
        },
        config_files: &["config.toml"],
        auth_probes: &[
            AuthProbe::JsonFile {
                relative_path: "auth.json",
                existence_field: "tokens",
                label: "ChatGPT OAuth",
            },
            AuthProbe::EnvKeys {
                vars: &["CODEX_API_KEY", "CODEX_ACCESS_TOKEN", "OPENAI_API_KEY"],
                label: "API key",
            },
        ],
        injection: Some(&CODEX_INJECTION),
        isolation: Some(&CODEX_ISOLATION),
        keychain_isolation: None,
        auth_login: AuthLoginSpec::Exec {
            label: "Codex",
            binary: "codex",
            args: &["auth", "login"],
        },
    },
    RuntimeSpec {
        name: "OpenCode",
        binary_names: &["opencode"],
        version_arg: "--version",
        config_locator: ConfigLocator::XdgConfig {
            subdir: "opencode",
            env_override: "OPENCODE_CONFIG_DIR",
        },
        config_files: &["opencode.jsonc", "opencode.json"],
        auth_probes: &[
            AuthProbe::DataDirJsonFile {
                data_subdir: "opencode",
                file_name: "auth.json",
                label: "Provider auth",
            },
            AuthProbe::EnvKeys {
                vars: &[
                    "ANTHROPIC_API_KEY",
                    "OPENAI_API_KEY",
                    "GOOGLE_API_KEY",
                    "GROQ_API_KEY",
                ],
                label: "API key",
            },
        ],
        injection: Some(&OPENCODE_INJECTION),
        isolation: Some(&OPENCODE_ISOLATION),
        keychain_isolation: None,
        auth_login: AuthLoginSpec::Message {
            lines: &[
                "OpenCode uses provider-specific authentication.",
                "Set API keys via environment variables:",
                "  export ANTHROPIC_API_KEY=sk-...",
                "  export OPENAI_API_KEY=sk-...",
                "Or run `opencode` to authenticate interactively.",
            ],
        },
    },
    RuntimeSpec {
        name: "Pi",
        binary_names: &["pi"],
        version_arg: "--version",
        config_locator: ConfigLocator::EnvOrHome {
            env_var: "PI_CODING_AGENT_DIR",
            home_relative: ".pi/agent",
        },
        config_files: &["settings.json"],
        auth_probes: &[AuthProbe::JsonFile {
            relative_path: "auth.json",
            existence_field: "",
            label: "Auth token",
        }],
        injection: None,
        isolation: Some(&PI_ISOLATION),
        keychain_isolation: None,
        auth_login: AuthLoginSpec::Unsupported,
    },
];

#[cfg(test)]
#[path = "registry/tests.rs"]
mod tests;
