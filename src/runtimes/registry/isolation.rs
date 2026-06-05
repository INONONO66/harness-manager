use crate::runtimes::types::{IsolationSpec, SeedFile};

pub(super) static CLAUDE_ISOLATION: IsolationSpec = IsolationSpec {
    subdir: "claude",
    spoof_home: true,
    home_subdirs: &[".claude"],
    static_envs: &[
        ("CLAUDE_CONFIG_DIR", "{home}/.claude"),
        ("CLAUDE_CODE_TMPDIR", "{tmp}"),
        ("CLAUDE_CODE_DEBUG_LOGS_DIR", "{runtime_logs}"),
        ("DISABLE_UPDATES", "1"),
        ("DISABLE_AUTOUPDATER", "1"),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
        (
            "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL",
            "1",
        ),
        ("ENABLE_CLAUDEAI_MCP_SERVERS", "false"),
        ("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1"),
        ("DISABLE_LOGIN_COMMAND", "1"),
    ],
    seed_files: &[
        SeedFile {
            path: "{home}/.claude/settings.json",
            content: concat!(
                "{\n",
                "  \"apiKeyHelper\": \"{state}/apikey.sh\",\n",
                "  \"permissions\": { \"defaultMode\": \"ask\" }\n",
                "}\n",
            ),
            overwrite: false,
            mode: None,
        },
        SeedFile {
            path: "{state}/apikey.sh",
            content: "#!/bin/sh\nexec hm secret get claude-api-key\n",
            overwrite: true,
            mode: Some(0o700),
        },
    ],
    caveat: Some(
        "Claude default mode uses apiKeyHelper and disables /login to avoid macOS Keychain. Run `printf '%s' '<key>' | hm secret set claude-api-key` before first use, or pass --allow-keychain for OAuth mode.",
    ),
};

pub static CLAUDE_KEYCHAIN_ISOLATION: IsolationSpec = IsolationSpec {
    subdir: "claude-keychain",
    spoof_home: true,
    home_subdirs: &[".claude"],
    static_envs: &[
        ("CLAUDE_CONFIG_DIR", "{home}/.claude"),
        ("CLAUDE_CODE_TMPDIR", "{tmp}"),
        ("CLAUDE_CODE_DEBUG_LOGS_DIR", "{runtime_logs}"),
        ("DISABLE_UPDATES", "1"),
        ("DISABLE_AUTOUPDATER", "1"),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
        (
            "CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL",
            "1",
        ),
        ("ENABLE_CLAUDEAI_MCP_SERVERS", "false"),
        ("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1"),
    ],
    seed_files: &[],
    caveat: Some(
        "Claude --allow-keychain mode permits OAuth and may read/write macOS Keychain entry 'Claude Code-credentials'. Use only when you explicitly want Claude subscription login.",
    ),
};

pub(super) static CODEX_ISOLATION: IsolationSpec = IsolationSpec {
    subdir: "codex",
    spoof_home: true,
    home_subdirs: &[".codex"],
    static_envs: &[("CODEX_HOME", "{home}/.codex")],
    seed_files: &[SeedFile {
        path: "{home}/.codex/config.toml",
        content: concat!(
            "analytics_enabled = false\n",
            "check_for_update_on_startup = false\n",
            "cli_auth_credentials_store = \"file\"\n",
            "mcp_oauth_credentials_store = \"file\"\n",
        ),
        overwrite: false,
        mode: None,
    }],
    caveat: None,
};

pub(super) static OPENCODE_ISOLATION: IsolationSpec = IsolationSpec {
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
    ],
    seed_files: &[],
    caveat: None,
};

pub(super) static PI_ISOLATION: IsolationSpec = IsolationSpec {
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
