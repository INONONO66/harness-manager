use std::process::Command;

use super::PackageSpec;
use crate::isolation::IsolationPaths;

pub(crate) fn apply_npm_isolated_env(
    cmd: &mut Command,
    spec: &PackageSpec,
    paths: &IsolationPaths,
) {
    if let PackageSpec::NpmIsolated { .. } = spec {
        let prefix = paths.home.join(".npm");
        let cache = paths.state.join("npm-cache");
        cmd.env("NPM_CONFIG_PREFIX", &prefix);
        cmd.env("NPM_CONFIG_CACHE", &cache);
        strip_shim_dirs_from_cmd_path(cmd);
    }
}

fn strip_shim_dirs_from_cmd_path(cmd: &mut Command) {
    let path_val: Option<String> = cmd.get_envs().find_map(|(k, v)| {
        if k == "PATH" {
            v.map(|val| val.to_string_lossy().to_string())
        } else {
            None
        }
    });
    let Some(path) = path_val else {
        return;
    };
    let filtered: Vec<&str> = path
        .split(':')
        .filter(|dir| !dir.contains("mise/shims") && !dir.contains("asdf/shims"))
        .collect();
    cmd.env("PATH", filtered.join(":"));
}
