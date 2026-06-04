use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::bail;
use colored::Colorize;

use crate::config;
use crate::harnesses;
use crate::harnesses::types::HarnessSpec;
use crate::isolation;
use crate::runtimes::registry::{CLAUDE_KEYCHAIN_ISOLATION, RUNTIMES};
use crate::runtimes::types::{IsolationSpec, RuntimeSpec};

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
    RUNTIMES
        .iter()
        .find(|r| r.name.to_lowercase() == lower || r.binary_names.iter().any(|b| *b == lower))
}

// ---------------------------------------------------------------------------
// Target resolution: harness-first, then runtime
// ---------------------------------------------------------------------------

/// What `hm use <name>` resolved to.
enum LaunchTarget {
    /// A plain runtime (existing behavior).
    Runtime(&'static RuntimeSpec),
    /// A harness wrapping an underlying runtime.
    Harness {
        harness: &'static HarnessSpec,
        runtime: &'static RuntimeSpec,
    },
}

/// Try harnesses first, then runtimes.
fn resolve_target(name: &str) -> anyhow::Result<LaunchTarget> {
    if let Some(h) = harnesses::find_harness_spec(name) {
        let rt = find_runtime_spec(h.target_runtime).ok_or_else(|| {
            anyhow::anyhow!(
                "harness '{}' targets runtime '{}', but that runtime is not registered",
                h.id,
                h.target_runtime
            )
        })?;
        return Ok(LaunchTarget::Harness {
            harness: h,
            runtime: rt,
        });
    }
    if let Some(rt) = find_runtime_spec(name) {
        return Ok(LaunchTarget::Runtime(rt));
    }
    bail!(
        "unknown target: '{}'. Run `hm detect` or `hm harness list` to see available targets.",
        name
    )
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run_use(
    target_name: &str,
    profile_name: Option<&str>,
    print_env: bool,
    allow_keychain: bool,
    extra_args: &[String],
) -> anyhow::Result<()> {
    let target = resolve_target(target_name)?;

    // Destructure into the pieces we need regardless of variant.
    let (spec, effective_isolation, binary_names, display_name): (
        &RuntimeSpec,
        Option<&IsolationSpec>,
        &[&str],
        String,
    ) = match &target {
        LaunchTarget::Runtime(rt) => {
            let iso = if allow_keychain && rt.name == "Claude Code" {
                Some(&CLAUDE_KEYCHAIN_ISOLATION as &IsolationSpec)
            } else {
                rt.isolation
            };
            (rt, iso, rt.binary_names, rt.name.to_string())
        }
        LaunchTarget::Harness { harness, runtime } => {
            if allow_keychain {
                bail!("--allow-keychain is not supported for harness launches");
            }
            // Harness always has isolation. The launch binary is either the
            // harness's own wrapper or the underlying runtime's binary.
            let bins: &[&str] = match &harness.launch_binary {
                Some(bin) => std::slice::from_ref(bin),
                None => runtime.binary_names,
            };
            let name = format!("{} ({})", harness.display_name, runtime.name);
            (
                *runtime,
                Some(&harness.isolation as &IsolationSpec),
                bins,
                name,
            )
        }
    };

    // --- Isolation setup ---------------------------------------------------
    let iso_setup = if let Some(iso) = effective_isolation {
        let paths = isolation::IsolationPaths::from_spec(iso);
        isolation::ensure_isolation_tree(iso, &paths)?;
        isolation::seed_files(iso, &paths)?;
        Some((iso, paths))
    } else {
        None
    };

    // --- Env: start from inherited, then strip + inject --------------------
    let mut env: HashMap<String, String> = std::env::vars().collect();

    for var in GLOBAL_AI_STRIP {
        env.remove(*var);
    }
    if let Some(injection) = spec.injection {
        for var in injection.strip_envs {
            env.remove(*var);
        }
    }

    if let Some((iso, ref paths)) = iso_setup {
        for (k, v) in isolation::build_isolation_env(iso, paths) {
            env.insert(k, v);
        }
        if let Some(caveat) = iso.caveat {
            eprintln!("{} {}", "⚠".yellow().bold(), caveat);
        }
    }

    // --- Profile injection (endpoint, bearer, proxy) -----------------------
    let profile_applied = if let Some(profile_arg) = profile_name {
        let hm_config = config::load_config()?;
        let resolved = config::resolve_profile(&hm_config, Some(profile_arg))?;

        if !print_env {
            eprintln!(
                "{} {} with profile '{}'",
                "Launching".green().bold(),
                display_name.bold(),
                resolved.name.cyan()
            );
        }

        if let Some(injection) = spec.injection {
            if let Some(ref endpoint) = resolved.endpoint {
                let effective_endpoint = if injection.endpoint_strip_v1 {
                    endpoint
                        .trim_end_matches('/')
                        .trim_end_matches("/v1")
                        .to_string()
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
        true
    } else {
        false
    };

    // --- print-env exit path -----------------------------------------------
    if print_env {
        let mut keys: Vec<&String> = env.keys().collect();
        keys.sort();
        for k in keys {
            println!("{}={}", k, env[k]);
        }
        return Ok(());
    }

    // --- Resolve binary and exec -------------------------------------------
    let binary = crate::runtimes::find_binary(binary_names).ok_or_else(|| {
        anyhow::anyhow!(
            "{} is not installed (binary not found in PATH)",
            display_name
        )
    })?;

    if !profile_applied {
        let suffix = if iso_setup.is_some() {
            "(isolated, no profile)"
        } else {
            "(no profile)"
        };
        eprintln!(
            "{} {} {}",
            "Launching".green().bold(),
            display_name.bold(),
            suffix
        );
    }

    let mut cmd = Command::new(&binary);
    cmd.args(extra_args);
    cmd.env_clear();
    for (k, v) in &env {
        cmd.env(k, v);
    }
    let err = cmd.exec();
    bail!("failed to exec {}: {}", binary.display(), err);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_target_runtime() {
        match resolve_target("codex").unwrap() {
            LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
            _ => panic!("expected Runtime"),
        }
    }

    #[test]
    fn resolve_target_runtime_by_name() {
        match resolve_target("Codex CLI").unwrap() {
            LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
            _ => panic!("expected Runtime"),
        }
    }

    #[test]
    fn resolve_target_harness() {
        match resolve_target("omx").unwrap() {
            LaunchTarget::Harness { harness, runtime } => {
                assert_eq!(harness.id, "omx");
                assert_eq!(runtime.name, "Codex CLI");
            }
            _ => panic!("expected Harness"),
        }
    }

    #[test]
    fn resolve_target_harness_case_insensitive() {
        match resolve_target("OMX").unwrap() {
            LaunchTarget::Harness { harness, .. } => assert_eq!(harness.id, "omx"),
            _ => panic!("expected Harness"),
        }
    }

    #[test]
    fn resolve_target_unknown() {
        assert!(resolve_target("nonexistent-xyz").is_err());
    }

    #[test]
    fn resolve_target_omc_targets_claude() {
        match resolve_target("omc").unwrap() {
            LaunchTarget::Harness { harness, runtime } => {
                assert_eq!(harness.id, "omc");
                assert_eq!(runtime.name, "Claude Code");
            }
            _ => panic!("expected Harness"),
        }
    }

    #[test]
    fn resolve_target_lazycodex_has_wrapper() {
        match resolve_target("lazycodex").unwrap() {
            LaunchTarget::Harness { harness, .. } => {
                assert_eq!(harness.launch_binary, Some("lazycodex-ai"));
            }
            _ => panic!("expected Harness"),
        }
    }
}
