//! Per-runtime isolation: build $HM tree, redirect env, seed config files.
//!
//! Applied by `launch::run_use` when `RuntimeSpec.isolation` is `Some`.
//! See `runtimes::types::IsolationSpec` for the declarative recipe shape.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs2::FileExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use spec::IsolationRecipe;

mod env;
mod paths;
pub mod spec;

#[cfg(test)]
pub use env::build_isolation_env;
pub use env::{build_sanitized_isolation_env, GLOBAL_AI_STRIP};
pub use paths::ensure_safe_write_path;
use paths::{
    create_private_dir_all, create_private_isolation_base, ensure_under_base, isolation_root,
    reject_existing_symlink_chain, validate_relative_path,
};

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
    pub runtime_base: PathBuf,
    pub runtime_home: PathBuf,
    pub runtime_state: PathBuf,
    pub runtime_logs: PathBuf,
}

impl IsolationPaths {
    pub fn try_from_spec(spec: &(impl IsolationRecipe + ?Sized)) -> Result<Self> {
        validate_relative_path(spec.subdir(), "isolation subdir")?;
        validate_relative_path(spec.runtime_subdir(), "runtime isolation subdir")?;
        let base = hm_data_dir().join("runtimes").join(spec.subdir());
        let runtime_base = hm_data_dir().join("runtimes").join(spec.runtime_subdir());
        Ok(Self {
            home: base.join("home"),
            state: base.join("state"),
            tmp: base.join("tmp"),
            base,
            runtime_home: runtime_base.join("home"),
            runtime_state: runtime_base.join("state"),
            runtime_logs: runtime_base.join("state").join("logs"),
            runtime_base,
        })
    }

    pub fn lock_file(&self) -> Result<PathBuf> {
        let runtime_root = self
            .base
            .parent()
            .with_context(|| format!("isolation base has no parent: {}", self.base.display()))?;
        let runtime_subdir = self.runtime_base.file_name().with_context(|| {
            format!("runtime base has no leaf: {}", self.runtime_base.display())
        })?;
        Ok(runtime_root
            .join(".locks")
            .join(format!("{}.lock", runtime_subdir.to_string_lossy())))
    }
}

pub struct IsolationLockGuard {
    _file: fs::File,
}

impl IsolationLockGuard {
    pub fn acquire(paths: &IsolationPaths) -> Result<Self> {
        let root = isolation_root(paths)?;
        let lock_file = paths.lock_file()?;
        let lock_dir = lock_file
            .parent()
            .with_context(|| format!("lock file has no parent: {}", lock_file.display()))?;
        ensure_under_base(lock_dir, &root, "isolation lock dir")?;
        create_private_dir_all(lock_dir, &root, "isolation lock dir")?;
        ensure_under_base(&lock_file, &root, "isolation lock")?;
        reject_existing_symlink_chain(&lock_file, &root, "isolation lock")?;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_file)
            .with_context(|| format!("open isolation lock {}", lock_file.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("lock isolation {}", lock_file.display()))?;
        Ok(Self { _file: file })
    }
}

/// Substitute `{home}`, `{state}`, `{tmp}` tokens in a template string.
/// Unknown tokens (e.g. `{foo}`) pass through unchanged.
pub fn subst_tokens(template: &str, paths: &IsolationPaths) -> String {
    template
        .replace("{home}", &paths.home.to_string_lossy())
        .replace("{state}", &paths.state.to_string_lossy())
        .replace("{tmp}", &paths.tmp.to_string_lossy())
        .replace("{runtime_home}", &paths.runtime_home.to_string_lossy())
        .replace("{runtime_state}", &paths.runtime_state.to_string_lossy())
        .replace("{runtime_logs}", &paths.runtime_logs.to_string_lossy())
}

/// Create the isolation tree: `home/`, `state/`, `tmp/`, plus all `home_subdirs`.
pub fn ensure_isolation_tree(
    spec: &(impl IsolationRecipe + ?Sized),
    paths: &IsolationPaths,
) -> Result<()> {
    let root = isolation_root(paths)?;
    create_private_isolation_base(&paths.base, &root)?;
    create_private_dir_all(&paths.home, &paths.base, "isolation home")?;
    create_private_dir_all(&paths.state, &paths.base, "isolation state")?;
    create_private_dir_all(&paths.tmp, &paths.base, "isolation tmp")?;
    create_private_isolation_base(&paths.runtime_base, &root)?;
    create_private_dir_all(&paths.runtime_home, &paths.runtime_base, "runtime home")?;
    create_private_dir_all(&paths.runtime_state, &paths.runtime_base, "runtime state")?;
    create_private_dir_all(&paths.runtime_logs, &paths.runtime_base, "runtime logs")?;
    for sub in spec.home_subdirs() {
        if sub.is_empty() {
            continue;
        }
        validate_relative_path(sub, "home subdir")?;
        let p = paths.home.join(sub);
        create_private_dir_all(&p, &paths.home, "home subdir")?;
    }
    Ok(())
}

/// Seed config files declared in `spec.seed_files`. Policy: create-if-missing.
/// User edits to seeded files are preserved across launches.
/// To reset a runtime to seed defaults, use `hm purge <runtime>` (Phase 3).
pub fn seed_files(spec: &(impl IsolationRecipe + ?Sized), paths: &IsolationPaths) -> Result<()> {
    for seed in spec.seed_files() {
        let path = PathBuf::from(subst_tokens(seed.path, paths));
        let seed_base = seed_trusted_base(&path, paths)?;
        ensure_under_base(&path, seed_base, "seed file")?;
        reject_existing_symlink_chain(&path, seed_base, "seed file")?;
        if path.exists() && !seed.overwrite {
            continue;
        }
        if let Some(parent) = path.parent() {
            create_private_dir_all(parent, seed_base, "seed parent")?;
        }
        let content = subst_tokens(seed.content, paths);
        write_seed_file(&path, content.as_bytes(), seed.overwrite, seed.mode)?;
    }
    Ok(())
}

fn seed_trusted_base<'a>(path: &Path, paths: &'a IsolationPaths) -> Result<&'a Path> {
    if path.starts_with(&paths.base) {
        return Ok(&paths.base);
    }
    if path.starts_with(&paths.runtime_base) {
        return Ok(&paths.runtime_base);
    }
    anyhow::bail!(
        "seed file must stay under {} or {}",
        paths.base.display(),
        paths.runtime_base.display()
    )
}

pub fn purge_isolation_tree(paths: &IsolationPaths) -> Result<()> {
    let root = isolation_root(paths)?;
    ensure_under_base(&paths.base, &root, "isolation purge")?;
    reject_existing_symlink_chain(&paths.base, &root, "isolation purge")?;
    match fs::symlink_metadata(&paths.base) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!(
                "isolation purge must not traverse symlink {}",
                paths.base.display()
            );
        }
        Ok(metadata) if !metadata.is_dir() => {
            anyhow::bail!(
                "isolation purge target is not a directory: {}",
                paths.base.display()
            );
        }
        Ok(_) => {
            fs::remove_dir_all(&paths.base)
                .with_context(|| format!("failed to purge {}", paths.base.display()))?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).with_context(|| format!("inspect {}", paths.base.display())),
    }
    Ok(())
}

fn temp_seed_path(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .with_context(|| format!("seed path has no parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("seed path has invalid file name: {}", path.display()))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_nanos();
    Ok(parent.join(format!(
        ".{}.hm-seed-{}-{}.tmp",
        file_name,
        std::process::id(),
        nanos
    )))
}

fn write_seed_file(path: &Path, content: &[u8], overwrite: bool, mode: Option<u32>) -> Result<()> {
    if path.exists() && !overwrite {
        return Ok(());
    }
    let tmp = temp_seed_path(path)?;
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .with_context(|| format!("create temp seed {}", tmp.display()))?;
        file.write_all(content)
            .with_context(|| format!("write temp seed {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("sync temp seed {}", tmp.display()))?;
        drop(file);
        #[cfg(unix)]
        if let Some(mode) = mode {
            fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))
                .with_context(|| format!("chmod {:o} {}", mode, tmp.display()))?;
        }
        if overwrite {
            fs::rename(&tmp, path).with_context(|| format!("replace seed {}", path.display()))?;
        } else if let Err(err) = fs::hard_link(&tmp, path) {
            if err.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(err).with_context(|| format!("link seed {}", path.display()));
            }
        }
        Ok(())
    })();
    let _ = fs::remove_file(&tmp);
    result
}

#[cfg(test)]
mod tests;
