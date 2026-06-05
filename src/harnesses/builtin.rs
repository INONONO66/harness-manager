#![allow(dead_code)]

use super::manifest::{parse_toml, ManifestHarnessSpec};

pub const BUILTIN_MANIFESTS: &[(&str, &str)] = &[
    ("builtin/omc.toml", include_str!("builtin/omc.toml")),
    ("builtin/omx.toml", include_str!("builtin/omx.toml")),
    ("builtin/omo.toml", include_str!("builtin/omo.toml")),
    (
        "builtin/lazycodex.toml",
        include_str!("builtin/lazycodex.toml"),
    ),
    (
        "builtin/ouroboros.toml",
        include_str!("builtin/ouroboros.toml"),
    ),
];

pub fn builtin_specs() -> anyhow::Result<Vec<ManifestHarnessSpec>> {
    BUILTIN_MANIFESTS
        .iter()
        .map(|(path, input)| parse_toml(path, input))
        .collect()
}

#[cfg(test)]
#[path = "builtin_tests.rs"]
mod tests;
