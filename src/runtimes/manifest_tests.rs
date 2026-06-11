use super::{
    parse_toml, AuthLoginRecord, AuthProbeRecord, ConfigLocatorRecord, InjectionRecord,
    RuntimeRecord,
};
use crate::runtimes::builtin::builtin_runtime_records;

fn parse_for(name: &str) -> RuntimeRecord {
    let records = builtin_runtime_records().expect("builtins parse");
    records
        .into_iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("builtin runtime '{name}' not found"))
}

#[test]
fn claude_manifest_parses_with_env_injection_and_keychain_variant() {
    let claude = parse_for("Claude Code");

    assert_eq!(claude.binary_names, vec!["claude".to_string()]);
    assert_eq!(claude.version_arg, "--version");

    match claude.config_locator {
        ConfigLocatorRecord::EnvOrHome {
            ref env,
            ref home_relative,
        } => {
            assert_eq!(env, "CLAUDE_CONFIG_DIR");
            assert_eq!(home_relative, ".claude");
        }
        _ => panic!("expected EnvOrHome"),
    }

    let env_probe = claude
        .auth_probes
        .iter()
        .find(|p| matches!(p, AuthProbeRecord::EnvKeys { .. }))
        .expect("env-keys probe");
    if let AuthProbeRecord::EnvKeys { vars, .. } = env_probe {
        assert!(vars.contains(&"ANTHROPIC_API_KEY".to_string()));
    }

    match claude.auth_login {
        AuthLoginRecord::Exec {
            ref binary,
            ref args,
            ..
        } => {
            assert_eq!(binary, "claude");
            assert!(args.is_empty());
        }
        _ => panic!("expected exec login"),
    }

    let injection = claude.injection.expect("injection present");
    match injection {
        InjectionRecord::Env(env) => {
            assert_eq!(env.provider, "anthropic");
            assert_eq!(env.endpoint_env, "ANTHROPIC_BASE_URL");
            assert_eq!(env.api_key_env, "ANTHROPIC_API_KEY");
            assert!(env.endpoint_strip_v1);
            assert!(env.strip_envs.contains(&"ANTHROPIC_API_KEY".to_string()));
            assert!(env.supported_providers.contains(&"anthropic".to_string()));
        }
        _ => panic!("expected env strategy"),
    }

    let isolation = claude.isolation.expect("isolation present");
    assert_eq!(isolation.subdir, "claude");
    assert!(isolation.spoof_home);
    assert!(isolation
        .static_envs
        .iter()
        .any(|(k, v)| k == "CLAUDE_CONFIG_DIR" && v == "{home}/.claude"));
    assert_eq!(isolation.seed_files.len(), 2);

    let keychain = claude
        .keychain_isolation
        .expect("keychain isolation present");
    assert_eq!(keychain.subdir, "claude-keychain");
    assert!(keychain.seed_files.is_empty());
}

#[test]
fn codex_manifest_parses_with_codex_config_seed_strategy() {
    let codex = parse_for("Codex CLI");
    let injection = codex.injection.expect("injection present");
    match injection {
        InjectionRecord::CodexConfigSeed(spec) => {
            assert_eq!(spec.config_path, "{home}/.codex/config.toml");
            assert_eq!(spec.openai_base_url_key, "openai_base_url");
            assert_eq!(spec.model_provider_key, "model_provider");
            assert_eq!(spec.model_provider_value, "openai");
            assert_eq!(spec.provider, "openai");
            assert_eq!(spec.supported_providers, vec!["openai".to_string()]);
            assert_eq!(spec.api_key_env, "CODEX_API_KEY");
            assert!(spec.strip_envs.contains(&"OPENAI_API_KEY".to_string()));
            assert!(spec.strip_envs.contains(&"OPENAI_BASE_URL".to_string()));
            assert!(spec.strip_envs.contains(&"CODEX_API_KEY".to_string()));
            assert!(spec.strip_envs.contains(&"CODEX_ACCESS_TOKEN".to_string()));
            assert!(!spec.overwrite);
            assert!(!spec.endpoint_strip_v1);
        }
        _ => panic!("expected codex-config-seed strategy"),
    }
}

#[test]
fn runtime_manifest_parses_shared_state_policy() {
    let input = r#"
schema_version = 1
name = "Custom Codex"
binary_names = ["custom-codex"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = "CODEX_HOME"
home_relative = ".codex"

[auth_login]
kind = "unsupported"

[shared_state]
database_dirs = [".codex"]
auth_files = [".codex/auth.json"]
"#;

    let parsed = parse_toml("custom-codex.toml", input).expect("shared_state parses");
    let shared_state = parsed.shared_state.expect("shared state policy");

    assert_eq!(shared_state.database_dirs, vec![".codex"]);
    assert_eq!(shared_state.auth_files, vec![".codex/auth.json"]);
}

#[test]
fn rejects_shared_state_path_traversal() {
    let input = r#"
schema_version = 1
name = "Bad Shared State"
binary_names = ["bad-shared-state"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".bad"

[auth_login]
kind = "unsupported"

[shared_state]
database_dirs = [".bad"]
auth_files = ["../secret.json"]
"#;

    let err = parse_toml("bad-shared-state.toml", input)
        .expect_err("traversing shared_state path must fail");

    assert!(
        err.to_string().contains("shared_state.auth_files"),
        "expected shared_state.auth_files validation error, got: {err:#}"
    );
}

#[test]
fn codex_manifest_separates_access_token_env_probe_from_api_key() {
    let codex = parse_for("Codex CLI");

    let env_probes: Vec<(Vec<String>, String)> = codex
        .auth_probes
        .iter()
        .filter_map(|p| match p {
            AuthProbeRecord::EnvKeys { vars, label } => Some((vars.clone(), label.clone())),
            _ => None,
        })
        .collect();

    let api_key_probe = env_probes
        .iter()
        .find(|(_, label)| label == "API key")
        .expect("API key env probe present");
    assert!(api_key_probe.0.contains(&"CODEX_API_KEY".to_string()));
    assert!(api_key_probe.0.contains(&"OPENAI_API_KEY".to_string()));
    assert!(
        !api_key_probe.0.contains(&"CODEX_ACCESS_TOKEN".to_string()),
        "CODEX_ACCESS_TOKEN must not be lumped under 'API key' (upstream codex loads it as agent identity / access bearer, not an API key)"
    );

    let access_token_probe = env_probes
        .iter()
        .find(|(vars, _)| vars.contains(&"CODEX_ACCESS_TOKEN".to_string()))
        .expect("CODEX_ACCESS_TOKEN env probe present");
    assert_eq!(
        access_token_probe.1, "Access token",
        "CODEX_ACCESS_TOKEN probe must use the distinct 'Access token' label (probe_env_keys appends '(VAR)' to match the API key formatting)"
    );
    assert_eq!(
        access_token_probe.0,
        vec!["CODEX_ACCESS_TOKEN".to_string()],
        "Access token probe must contain ONLY CODEX_ACCESS_TOKEN (don't merge with other auth env vars)"
    );
}

#[test]
fn opencode_manifest_parses_with_provider_config_seed() {
    let opencode = parse_for("OpenCode");
    let injection = opencode.injection.expect("injection present");
    match injection {
        InjectionRecord::ProviderConfigSeed(seed) => {
            assert_eq!(seed.root_key, "provider");
            assert_eq!(seed.provider_base_url_key, "options.baseURL");
            assert_eq!(seed.provider_api_key_key, "options.apiKey");
            assert_eq!(
                seed.provider_headers_key.as_deref(),
                Some("options.headers")
            );
            assert!(seed.supported_providers.contains(&"anthropic".to_string()));
            assert!(seed.supported_providers.contains(&"openai".to_string()));
            assert!(seed.supported_providers.contains(&"google".to_string()));
            assert!(seed.config_path.ends_with(".config/opencode/opencode.json"));
            let anthropic_headers = seed
                .provider_header_overrides
                .get("anthropic")
                .expect("anthropic header overrides");
            assert_eq!(
                anthropic_headers.get("x-api-key").map(String::as_str),
                Some("{bearer}")
            );
        }
        _ => panic!("expected provider-config-seed strategy"),
    }
}

#[test]
fn pi_manifest_parses_with_provider_config_seed_and_unsupported_login() {
    let pi = parse_for("Pi");
    assert!(matches!(pi.auth_login, AuthLoginRecord::Unsupported));
    let injection = pi.injection.expect("injection present");
    match injection {
        InjectionRecord::ProviderConfigSeed(seed) => {
            assert_eq!(seed.root_key, "providers");
            assert_eq!(seed.provider_base_url_key, "baseUrl");
            assert_eq!(seed.provider_api_key_key, "apiKey");
            assert_eq!(seed.provider_headers_key.as_deref(), Some("headers"));
            assert!(seed.config_path.ends_with(".pi/agent/models.json"));
        }
        _ => panic!("expected provider-config-seed strategy"),
    }
}

#[test]
fn parser_rejects_unknown_top_level_field() {
    let input = r#"
schema_version = 1
name = "Bogus"
binary_names = ["bogus"]
version_arg = "--version"
unexpected = "field"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".bogus"

[auth_login]
kind = "unsupported"
"#;
    let err = parse_toml("bogus.toml", input).expect_err("unknown field must fail");
    assert!(
        err.to_string().to_lowercase().contains("unknown")
            || err.to_string().to_lowercase().contains("failed to parse"),
        "expected parse error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_seed_outside_home() {
    let input = r#"
schema_version = 1
name = "Bad"
binary_names = ["bad"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".bad"

[auth_login]
kind = "unsupported"

[isolation]
subdir = "bad"
spoof_home = true

[[isolation.seed_files]]
path = "/etc/passwd"
content = "x"
overwrite = false
"#;
    let err = parse_toml("bad.toml", input).expect_err("absolute seed path must fail");
    assert!(
        err.to_string().contains("seed_files"),
        "expected seed_files error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_unknown_injection_strategy() {
    let input = r#"
schema_version = 1
name = "Bad"
binary_names = ["bad"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".bad"

[auth_login]
kind = "unsupported"

[injection]
strategy = "magic"
"#;
    let err = parse_toml("bad.toml", input).expect_err("unknown strategy must fail");
    assert!(
        err.to_string().to_lowercase().contains("parse")
            || err.to_string().to_lowercase().contains("variant"),
        "expected variant error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_provider_config_seed_outside_home() {
    let input = r#"
schema_version = 1
name = "Bad"
binary_names = ["bad"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".bad"

[auth_login]
kind = "unsupported"

[injection]
strategy = "provider-config-seed"
config_path = "/etc/passwd"
root_key = "providers"
provider_base_url_key = "baseUrl"
provider_api_key_key = "apiKey"
supported_providers = ["anthropic"]
"#;
    let err = parse_toml("bad.toml", input).expect_err("config path must start with {home}/");
    assert!(
        err.to_string().contains("config_path"),
        "expected config_path error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_anthropic_header_with_colon_in_name() {
    let input = r#"
schema_version = 1
name = "Bad"
binary_names = ["bad"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".bad"

[auth_login]
kind = "unsupported"

[injection]
strategy = "provider-config-seed"
config_path = "{home}/.bad/x.json"
root_key = "providers"
provider_base_url_key = "baseUrl"
provider_api_key_key = "apiKey"
supported_providers = ["anthropic"]

[injection.provider_header_overrides.anthropic]
"bad:name" = "{bearer}"
"#;
    let err = parse_toml("bad.toml", input).expect_err("colon in header name must fail");
    assert!(
        err.to_string().contains("provider_header_overrides"),
        "expected header error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_codex_config_seed_with_dotted_key() {
    let input = r#"
schema_version = 1
name = "BadCodex"
binary_names = ["badcodex"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".badcodex"

[auth_login]
kind = "unsupported"

[injection]
strategy = "codex-config-seed"
config_path = "{home}/.badcodex/config.toml"
openai_base_url_key = "a.b"
model_provider_key = "model_provider"
model_provider_value = "openai"
provider = "openai"
supported_providers = ["openai"]
api_key_env = "CODEX_API_KEY"
overwrite = false
endpoint_strip_v1 = false
"#;
    let err = parse_toml("badcodex.toml", input)
        .expect_err("dotted key in openai_base_url_key must fail");
    assert!(
        err.to_string().contains("openai_base_url_key"),
        "expected openai_base_url_key error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_codex_config_seed_with_provider_not_in_supported() {
    let input = r#"
schema_version = 1
name = "BadCodex"
binary_names = ["badcodex"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".badcodex"

[auth_login]
kind = "unsupported"

[injection]
strategy = "codex-config-seed"
config_path = "{home}/.badcodex/config.toml"
openai_base_url_key = "openai_base_url"
model_provider_key = "model_provider"
model_provider_value = "openai"
provider = "openai"
supported_providers = ["anthropic"]
api_key_env = "CODEX_API_KEY"
overwrite = false
endpoint_strip_v1 = false
"#;
    let err = parse_toml("badcodex.toml", input).expect_err("provider not in supported must fail");
    assert!(
        err.to_string().contains("supported_providers"),
        "expected supported_providers error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_codex_config_seed_with_bad_env_name() {
    let input = r#"
schema_version = 1
name = "BadCodex"
binary_names = ["badcodex"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".badcodex"

[auth_login]
kind = "unsupported"

[injection]
strategy = "codex-config-seed"
config_path = "{home}/.badcodex/config.toml"
openai_base_url_key = "openai_base_url"
model_provider_key = "model_provider"
model_provider_value = "openai"
provider = "openai"
supported_providers = ["openai"]
api_key_env = "1bad"
overwrite = false
endpoint_strip_v1 = false
"#;
    let err =
        parse_toml("badcodex.toml", input).expect_err("lowercase/digit-leading env name must fail");
    assert!(
        err.to_string().contains("api_key_env"),
        "expected api_key_env error, got: {err:#}"
    );
}

#[test]
fn parser_rejects_codex_config_seed_outside_home() {
    let input = r#"
schema_version = 1
name = "BadCodex"
binary_names = ["badcodex"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".badcodex"

[auth_login]
kind = "unsupported"

[injection]
strategy = "codex-config-seed"
config_path = "/etc/passwd"
openai_base_url_key = "openai_base_url"
model_provider_key = "model_provider"
model_provider_value = "openai"
provider = "openai"
supported_providers = ["openai"]
api_key_env = "CODEX_API_KEY"
overwrite = false
endpoint_strip_v1 = false
"#;
    let err = parse_toml("badcodex.toml", input).expect_err("config_path must start with {home}/");
    assert!(
        err.to_string().contains("config_path"),
        "expected config_path error, got: {err:#}"
    );
}

#[test]
fn parser_accepts_legacy_keychain_heuristic_without_keychain_service() {
    let input = r#"
schema_version = 1
name = "Legacy"
binary_names = ["legacy"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".legacy"

[[auth_probes]]
kind = "keychain-heuristic"
marker_file = "settings.json"
label = "OAuth (macOS Keychain)"

[auth_login]
kind = "unsupported"
"#;
    let record = parse_toml("legacy.toml", input).expect("legacy manifest must parse");
    let probe = record
        .auth_probes
        .iter()
        .find(|p| matches!(p, AuthProbeRecord::KeychainHeuristic { .. }))
        .expect("keychain probe present");
    if let AuthProbeRecord::KeychainHeuristic {
        keychain_service, ..
    } = probe
    {
        assert!(
            keychain_service.is_none(),
            "legacy manifest with no keychain_service field parses as None"
        );
    }
}

#[test]
fn parser_rejects_empty_keychain_service_when_explicitly_set() {
    let input = r#"
schema_version = 1
name = "BadKeychain"
binary_names = ["badkc"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".badkc"

[[auth_probes]]
kind = "keychain-heuristic"
marker_file = "settings.json"
keychain_service = "   "
label = "OAuth (macOS Keychain)"

[auth_login]
kind = "unsupported"
"#;
    let err =
        parse_toml("badkc.toml", input).expect_err("whitespace-only keychain_service rejected");
    assert!(
        err.to_string().contains("keychain_service"),
        "expected keychain_service validation error, got: {err:#}"
    );
}
