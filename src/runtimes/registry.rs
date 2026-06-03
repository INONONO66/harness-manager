use super::types::*;

static CLAUDE_INJECTION: InjectionSpec = InjectionSpec {
    endpoint_env: "ANTHROPIC_BASE_URL",
    api_key_env: "ANTHROPIC_API_KEY",
    proxy_envs: &["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"],
    strip_envs: &[
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
    ],
    endpoint_strip_v1: true,
};

static CODEX_INJECTION: InjectionSpec = InjectionSpec {
    endpoint_env: "OPENAI_BASE_URL",
    api_key_env: "OPENAI_API_KEY",
    proxy_envs: &["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"],
    strip_envs: &[
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "CODEX_API_KEY",
        "CODEX_ACCESS_TOKEN",
    ],
    endpoint_strip_v1: false,
};

static OPENCODE_INJECTION: InjectionSpec = InjectionSpec {
    endpoint_env: "OPENAI_BASE_URL",
    api_key_env: "OPENAI_API_KEY",
    proxy_envs: &["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"],
    strip_envs: &[
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GOOGLE_API_KEY",
        "GROQ_API_KEY",
    ],
    endpoint_strip_v1: false,
};

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
    },
    RuntimeSpec {
        name: "Pi",
        binary_names: &["pi"],
        version_arg: "--version",
        config_locator: ConfigLocator::EnvOrHome {
            env_var: "",
            home_relative: ".pi/agent",
        },
        config_files: &["settings.json"],
        auth_probes: &[AuthProbe::JsonFile {
            relative_path: "auth.json",
            existence_field: "",
            label: "Auth token",
        }],
        injection: None,
    },
];
