//! Per-runtime isolation: build $HM tree, redirect env, seed config files.
//!
//! Applied by `launch::run_use` when `RuntimeSpec.isolation` is `Some`.
//! See `runtimes::types::IsolationSpec` for the declarative recipe shape.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::runtimes::types::IsolationSpec;

/// Resolve the `hm` data directory.
///
/// Honors `$XDG_DATA_HOME` directly (cross-platform contract: `$HM = $XDG_DATA_HOME/hm`).
/// Does NOT use `dirs::data_dir()` because on macOS that returns
/// `~/Library/Application Support`, which breaks the documented contract.
pub fn hm_data_dir() -> PathBuf {
    if let Ok(v) = std::env::var("XDG_DATA_HOME") {
        if !v.is_empty() {
            return PathBuf::from(v).join("hm");
        }
    }
    dirs::home_dir()
        .map(|h| h.join(".local").join("share").join("hm"))
        .unwrap_or_else(|| PathBuf::from(".local/share/hm"))
}

/// Per-runtime isolation paths under `$HM/runtimes/<subdir>/`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct IsolationPaths {
    pub base: PathBuf,
    pub home: PathBuf,
    pub state: PathBuf,
    pub tmp: PathBuf,
}

impl IsolationPaths {
    pub fn from_spec(spec: &IsolationSpec) -> Self {
        let base = hm_data_dir().join("runtimes").join(spec.subdir);
        Self {
            home: base.join("home"),
            state: base.join("state"),
            tmp: base.join("tmp"),
            base,
        }
    }
}

/// Substitute `{home}`, `{state}`, `{tmp}` tokens in a template string.
/// Unknown tokens (e.g. `{foo}`) pass through unchanged.
pub fn subst_tokens(template: &str, paths: &IsolationPaths) -> String {
    template
        .replace("{home}", &paths.home.to_string_lossy())
        .replace("{state}", &paths.state.to_string_lossy())
        .replace("{tmp}", &paths.tmp.to_string_lossy())
}

/// Create the isolation tree: `home/`, `state/`, `tmp/`, plus all `home_subdirs`.
pub fn ensure_isolation_tree(spec: &IsolationSpec, paths: &IsolationPaths) -> Result<()> {
    fs::create_dir_all(&paths.home).with_context(|| format!("create {}", paths.home.display()))?;
    fs::create_dir_all(&paths.state)
        .with_context(|| format!("create {}", paths.state.display()))?;
    fs::create_dir_all(&paths.tmp).with_context(|| format!("create {}", paths.tmp.display()))?;
    for sub in spec.home_subdirs {
        if sub.is_empty() {
            continue;
        }
        let p = paths.home.join(sub);
        fs::create_dir_all(&p).with_context(|| format!("create {}", p.display()))?;
    }
    Ok(())
}

/// Seed config files declared in `spec.seed_files`. Policy: create-if-missing.
/// User edits to seeded files are preserved across launches.
/// To reset a runtime to seed defaults, use `hm purge <runtime>` (Phase 3).
pub fn seed_files(spec: &IsolationSpec, paths: &IsolationPaths) -> Result<()> {
    for seed in spec.seed_files {
        let path = PathBuf::from(subst_tokens(seed.path, paths));
        if path.exists() && !seed.overwrite {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent {}", parent.display()))?;
        }
        let content = subst_tokens(seed.content, paths);
        fs::write(&path, content).with_context(|| format!("write seed {}", path.display()))?;
        #[cfg(unix)]
        if let Some(mode) = seed.mode {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .with_context(|| format!("chmod {:o} {}", mode, path.display()))?;
        }
    }
    Ok(())
}

/// Build the isolation env map: `HOME` (if spoof_home) + token-substituted `static_envs`.
pub fn build_isolation_env(
    spec: &IsolationSpec,
    paths: &IsolationPaths,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if spec.spoof_home {
        out.insert("HOME".to_string(), paths.home.to_string_lossy().to_string());
    }
    for (k, v_template) in spec.static_envs {
        out.insert(k.to_string(), subst_tokens(v_template, paths));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_paths(suffix: &str) -> IsolationPaths {
        let base = std::env::temp_dir().join(format!(
            "hm-iso-test-{}-{}-{}",
            std::process::id(),
            suffix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        IsolationPaths {
            home: base.join("home"),
            state: base.join("state"),
            tmp: base.join("tmp"),
            base,
        }
    }

    #[test]
    fn subst_tokens_replaces_home() {
        let p = tmp_paths("subst-home");
        let result = subst_tokens("{home}/.codex/config.toml", &p);
        assert!(
            result.ends_with("/home/.codex/config.toml"),
            "got: {}",
            result
        );
        assert!(!result.contains("{home}"));
    }

    #[test]
    fn subst_tokens_replaces_state_and_tmp() {
        let p = tmp_paths("subst-state-tmp");
        assert!(subst_tokens("{state}/logs", &p).ends_with("/state/logs"));
        assert!(subst_tokens("{tmp}/foo", &p).ends_with("/tmp/foo"));
    }

    #[test]
    fn subst_tokens_passes_through_unknown_and_plain() {
        let p = tmp_paths("subst-unknown");
        assert_eq!(subst_tokens("OPENCODE_PURE", &p), "OPENCODE_PURE");
        assert_eq!(subst_tokens("1", &p), "1");
        assert_eq!(subst_tokens("{unknown}/x", &p), "{unknown}/x");
    }

    #[test]
    fn ensure_tree_creates_home_subdirs() {
        let p = tmp_paths("ensure-tree");
        let _ = fs::remove_dir_all(&p.base);
        let spec = IsolationSpec {
            subdir: "test",
            spoof_home: true,
            home_subdirs: &[".codex", ".config/opencode"],
            static_envs: &[],
            seed_files: &[],
            caveat: None,
        };
        ensure_isolation_tree(&spec, &p).unwrap();
        assert!(p.home.join(".codex").is_dir());
        assert!(p.home.join(".config/opencode").is_dir());
        assert!(p.state.is_dir());
        assert!(p.tmp.is_dir());
        let _ = fs::remove_dir_all(&p.base);
    }

    #[test]
    fn seed_files_writes_substituted_content_create_if_missing() {
        let p = tmp_paths("seed-files");
        let _ = fs::remove_dir_all(&p.base);
        fs::create_dir_all(&p.home).unwrap();
        let spec = IsolationSpec {
            subdir: "test",
            spoof_home: true,
            home_subdirs: &[],
            static_envs: &[],
            seed_files: &[crate::runtimes::types::SeedFile {
                path: "{home}/.codex/config.toml",
                content: "home={home}\nanalytics_enabled = false\n",
                overwrite: false,
                mode: None,
            }],
            caveat: None,
        };
        seed_files(&spec, &p).unwrap();
        let path = p.home.join(".codex/config.toml");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("analytics_enabled = false"));
        assert!(content.contains(&p.home.to_string_lossy().to_string()));

        // create-if-missing: re-run does not overwrite user edits
        fs::write(&path, "USER_EDIT").unwrap();
        seed_files(&spec, &p).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "USER_EDIT");

        let _ = fs::remove_dir_all(&p.base);
    }

    #[test]
    fn seed_files_can_overwrite_and_chmod() {
        let p = tmp_paths("seed-overwrite");
        let _ = fs::remove_dir_all(&p.base);
        fs::create_dir_all(&p.state).unwrap();
        let spec = IsolationSpec {
            subdir: "test",
            spoof_home: true,
            home_subdirs: &[],
            static_envs: &[],
            seed_files: &[crate::runtimes::types::SeedFile {
                path: "{state}/apikey.sh",
                content: "#!/bin/sh\nexec hm secret get claude-api-key\n",
                overwrite: true,
                mode: Some(0o700),
            }],
            caveat: None,
        };
        let path = p.state.join("apikey.sh");
        fs::write(&path, "OLD").unwrap();
        seed_files(&spec, &p).unwrap();
        assert!(fs::read_to_string(&path)
            .unwrap()
            .contains("claude-api-key"));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let _ = fs::remove_dir_all(&p.base);
    }

    #[test]
    fn build_env_inserts_home_and_static_envs() {
        let p = tmp_paths("build-env");
        let spec = IsolationSpec {
            subdir: "test",
            spoof_home: true,
            home_subdirs: &[],
            static_envs: &[("CODEX_HOME", "{home}/.codex"), ("PI_OFFLINE", "1")],
            seed_files: &[],
            caveat: None,
        };
        let env = build_isolation_env(&spec, &p);
        assert_eq!(
            env.get("HOME").unwrap(),
            &p.home.to_string_lossy().to_string()
        );
        assert!(env.get("CODEX_HOME").unwrap().ends_with("/.codex"));
        assert_eq!(env.get("PI_OFFLINE").unwrap(), "1");
    }

    #[test]
    fn build_env_skips_home_when_spoof_disabled() {
        let p = tmp_paths("build-env-no-spoof");
        let spec = IsolationSpec {
            subdir: "test",
            spoof_home: false,
            home_subdirs: &[],
            static_envs: &[("FOO", "bar")],
            seed_files: &[],
            caveat: None,
        };
        let env = build_isolation_env(&spec, &p);
        assert!(env.get("HOME").is_none());
        assert_eq!(env.get("FOO").unwrap(), "bar");
    }
}
