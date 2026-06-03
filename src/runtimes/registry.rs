use super::types::*;

// ---------------------------------------------------------------------------
// Isolation specs (Phase 1: Codex / OpenCode / Pi).
// Claude is Phase 2 (apiKeyHelper + --allow-keychain mode).
// ---------------------------------------------------------------------------

static CODEX_ISOLATION: IsolationSpec = IsolationSpec {
    subdir: "codex",
    spoof_home: true,
    home_subdirs: &[".codex"],
    static_envs: &[("CODEX_HOME", "{home}/.codex")],
    seed_files: &[(
        "{home}/.codex/config.toml",
        concat!(
            "analytics_enabled = false\n",
            "check_for_update_on_startup = false\n",
            "cli_auth_credentials_store = \"file\"\n",
            "mcp_oauth_credentials_store = \"file\"\n",
        ),
    )],
    caveat: None,
};

static OPENCODE_ISOLATION: IsolationSpec = IsolationSpec {
    subdir: "opencode",
    spoof_home: true,
    home_subdirs: &[
        ".config/opencode",
        ".local/share/opencode",
        ".cache/opencode",
        ".local/state/opencode",
    ],
    static_envs: &[
        ("XDG_CONFIG_HOME", "{home}/.config"),
        ("XDG_DATA_HOME", "{home}/.local/share"),
        ("XDG_CACHE_HOME", "{home}/.cache"),
        ("XDG_STATE_HOME", "{home}/.local/state"),
        ("OPENCODE_DISABLE_AUTOUPDATE", "1"),
        ("OPENCODE_DISABLE_PROJECT_CONFIG", "1"),
        ("OPENCODE_PURE", "1"),
    ],
    seed_files: &[],
    caveat: None,
};

static PI_ISOLATION: IsolationSpec = IsolationSpec {
    subdir: "pi",
    spoof_home: true,
    home_subdirs: &[".pi/agent"],
    static_envs: &[
        ("PI_CODING_AGENT_DIR", "{home}/.pi/agent"),
        ("PI_OFFLINE", "1"),
        ("PI_SKIP_VERSION_CHECK", "1"),
        ("PI_TELEMETRY", "0"),
    ],
    seed_files: &[],
    caveat: None,
};

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
        isolation: None,
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
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_env_var_is_not_empty() {
        let pi = RUNTIMES.iter().find(|r| r.name == "Pi").expect("Pi runtime");
        match &pi.config_locator {
            ConfigLocator::EnvOrHome { env_var, .. } => {
                assert_eq!(*env_var, "PI_CODING_AGENT_DIR");
            }
            other => panic!("Pi config_locator changed shape: {:?}", other),
        }
    }

    #[test]
    fn phase1_runtimes_have_isolation() {
        for name in &["Codex CLI", "OpenCode", "Pi"] {
            let rt = RUNTIMES
                .iter()
                .find(|r| r.name == *name)
                .unwrap_or_else(|| panic!("{} runtime missing", name));
            assert!(
                rt.isolation.is_some(),
                "{} must have isolation in Phase 1",
                name
            );
        }
    }

    #[test]
    fn claude_has_no_isolation_in_phase_1() {
        let c = RUNTIMES
            .iter()
            .find(|r| r.name == "Claude Code")
            .expect("Claude Code runtime");
        assert!(
            c.isolation.is_none(),
            "Claude isolation is deferred to Phase 2"
        );
    }

    #[test]
    fn codex_isolation_has_seed_config() {
        let codex = RUNTIMES
            .iter()
            .find(|r| r.name == "Codex CLI")
            .expect("Codex CLI runtime");
        let iso = codex.isolation.expect("isolation set");
        assert_eq!(iso.subdir, "codex");
        assert!(iso.spoof_home);
        assert!(iso.seed_files.iter().any(|(p, c)| p.contains("config.toml")
            && c.contains("analytics_enabled = false")));
    }

    #[test]
    fn opencode_isolation_redirects_xdg_quartet() {
        let oc = RUNTIMES
            .iter()
            .find(|r| r.name == "OpenCode")
            .expect("OpenCode runtime");
        let iso = oc.isolation.expect("isolation set");
        for xdg in &[
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "XDG_STATE_HOME",
        ] {
            assert!(
                iso.static_envs.iter().any(|(k, _)| k == xdg),
                "OpenCode isolation missing {}",
                xdg
            );
        }
    }
}
