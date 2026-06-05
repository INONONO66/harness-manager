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
fn codex_manifest_parses_with_env_strategy() {
    let codex = parse_for("Codex CLI");
    let injection = codex.injection.expect("injection present");
    match injection {
        InjectionRecord::Env(env) => {
            assert_eq!(env.provider, "openai");
            assert_eq!(env.endpoint_env, "OPENAI_BASE_URL");
            assert!(!env.endpoint_strip_v1);
            assert!(env.strip_envs.contains(&"CODEX_API_KEY".to_string()));
        }
        _ => panic!("expected env strategy"),
    }
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
