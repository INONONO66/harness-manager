use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::bail;
use colored::Colorize;

use crate::config;
use crate::runtimes::registry::RUNTIMES;
use crate::runtimes::types::RuntimeSpec;

const GLOBAL_AI_STRIP: &[&str] = &[
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

fn find_runtime_spec(name: &str) -> Option<&'static RuntimeSpec> {
    let lower = name.to_lowercase();
    RUNTIMES.iter().find(|r| {
        r.name.to_lowercase() == lower
            || r.binary_names.iter().any(|b| *b == lower)
    })
}

pub fn run_use(runtime: &str, profile_name: Option<&str>, extra_args: &[String]) -> anyhow::Result<()> {
    let spec = find_runtime_spec(runtime)
        .ok_or_else(|| anyhow::anyhow!("unknown runtime: '{}'. Run `hm detect` to see available runtimes.", runtime))?;

    let binary = crate::runtimes::find_binary(spec.binary_names)
        .ok_or_else(|| anyhow::anyhow!("{} is not installed (binary not found in PATH)", spec.name))?;

    let mut cmd = Command::new(&binary);
    cmd.args(extra_args);

    if let Some(profile_arg) = profile_name {
        let hm_config = config::load_config()?;
        let resolved = config::resolve_profile(&hm_config, Some(profile_arg))?;

        eprintln!("{} {} with profile '{}'",
            "Launching".green().bold(),
            spec.name.bold(),
            resolved.name.cyan()
        );

        let mut env: HashMap<String, String> = std::env::vars().collect();

        for var in GLOBAL_AI_STRIP {
            env.remove(*var);
        }
        if let Some(ref injection) = spec.injection {
            for var in injection.strip_envs {
                env.remove(*var);
            }
        }

        if let Some(ref injection) = spec.injection {
            if let Some(ref endpoint) = resolved.endpoint {
                let effective_endpoint = if injection.endpoint_strip_v1 {
                    endpoint.trim_end_matches('/').trim_end_matches("/v1").to_string()
                } else {
                    endpoint.clone()
                };
                env.insert(injection.endpoint_env.to_string(), effective_endpoint);
            }

            if let Some(ref bearer) = resolved.bearer {
                env.insert(injection.api_key_env.to_string(), bearer.clone());
            }
        }

        if let Some(ref proxy) = resolved.http_proxy {
            env.insert("HTTP_PROXY".to_string(), proxy.clone());
            env.insert("http_proxy".to_string(), proxy.clone());
        }
        if let Some(ref proxy) = resolved.https_proxy {
            env.insert("HTTPS_PROXY".to_string(), proxy.clone());
            env.insert("https_proxy".to_string(), proxy.clone());
        }
        if let Some(ref no_proxy) = resolved.no_proxy {
            env.insert("NO_PROXY".to_string(), no_proxy.clone());
            env.insert("no_proxy".to_string(), no_proxy.clone());
        }

        cmd.env_clear();
        for (k, v) in &env {
            cmd.env(k, v);
        }
    } else {
        eprintln!("{} {} (no profile)",
            "Launching".green().bold(),
            spec.name.bold()
        );
    }

    let err = cmd.exec();
    bail!("failed to exec {}: {}", binary.display(), err);
}
