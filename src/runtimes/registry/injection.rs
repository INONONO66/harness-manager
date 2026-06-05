use crate::runtimes::types::InjectionSpec;

pub(super) static CLAUDE_INJECTION: InjectionSpec = InjectionSpec {
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

pub(super) static CODEX_INJECTION: InjectionSpec = InjectionSpec {
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

pub(super) static OPENCODE_INJECTION: InjectionSpec = InjectionSpec {
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
