use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

use crate::harnesses::spec::PackageCommandTemplate;
use crate::harnesses::types::HarnessSpec;
use crate::isolation::IsolationPaths;

fn checkout_path(paths: &IsolationPaths) -> PathBuf {
    paths.state.join("worktree")
}

fn checkout_cmd(repository: &str, worktree: &Path) -> Command {
    if worktree.join(".git").exists() {
        let mut cmd = Command::new("git");
        cmd.args(["-C", &worktree.to_string_lossy(), "pull", "--ff-only"]);
        cmd
    } else {
        let mut cmd = Command::new("git");
        cmd.args(["clone", "--single-branch", "--depth", "1", repository]);
        cmd.arg(worktree);
        cmd
    }
}

fn setup_cmd(worktree: &Path, template: &PackageCommandTemplate) -> Command {
    let (program, args) = template
        .argv
        .split_first()
        .expect("validated package command template is non-empty");
    let mut cmd = Command::new(worktree.join(program));
    cmd.args(args);
    cmd.current_dir(worktree);
    cmd
}

pub(super) fn prepare_package(
    repository: &str,
    template: &PackageCommandTemplate,
    action: &str,
    spec: &HarnessSpec,
    paths: &IsolationPaths,
) -> anyhow::Result<()> {
    fs::create_dir_all(&paths.state)
        .with_context(|| format!("create {}", paths.state.display()))?;
    let worktree = checkout_path(paths);
    let git_cmd = checkout_cmd(repository, &worktree);
    super::run_cmd_with_env(
        git_cmd,
        action,
        &spec.id,
        spec.target_runtime_shared_state.as_ref(),
        &spec.isolation,
        paths,
    )?;
    let setup_cmd = setup_cmd(&worktree, template);
    super::run_cmd_with_env(
        setup_cmd,
        action,
        &spec.id,
        spec.target_runtime_shared_state.as_ref(),
        &spec.isolation,
        paths,
    )
}
