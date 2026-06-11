use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::runtimes::builtin::BUILTIN_RUNTIME_MANIFESTS;
use crate::runtimes::manifest::RuntimeRecord;

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ManifestOrigin {
    Builtin,
    User,
}

pub(super) struct LoadedRuntime {
    pub(super) record: RuntimeRecord,
    pub(super) routes: HashSet<String>,
    pub(super) origin: ManifestOrigin,
    pub(super) content: String,
}

#[derive(Debug, Clone)]
pub enum RuntimeSource {
    Builtins,
    #[cfg(test)]
    Manifest {
        label: String,
        content: String,
    },
    File(PathBuf),
}

impl RuntimeSource {
    pub fn builtins() -> Self {
        Self::Builtins
    }

    #[cfg(test)]
    pub fn manifest(label: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Manifest {
            label: label.into(),
            content: content.into(),
        }
    }

    pub(super) fn origin(&self) -> ManifestOrigin {
        match self {
            Self::Builtins => ManifestOrigin::Builtin,
            #[cfg(test)]
            Self::Manifest { .. } => ManifestOrigin::User,
            Self::File(_) => ManifestOrigin::User,
        }
    }

    pub(super) fn contents(&self) -> Result<Vec<(String, String)>> {
        match self {
            Self::Builtins => Ok(BUILTIN_RUNTIME_MANIFESTS
                .iter()
                .map(|(label, content)| ((*label).to_string(), (*content).to_string()))
                .collect()),
            #[cfg(test)]
            Self::Manifest { label, content } => Ok(vec![(label.clone(), content.clone())]),
            Self::File(path) => {
                ensure_manifest_size(path, "runtime")?;
                let content = fs::read_to_string(path).with_context(|| {
                    format!("failed to read runtime manifest {}", path.display())
                })?;
                Ok(vec![(path.display().to_string(), content)])
            }
        }
    }
}

fn ensure_manifest_size(path: &Path, kind: &str) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to stat {kind} manifest {}", path.display()))?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        anyhow::bail!("{}: {kind} manifest exceeds 64 KiB", path.display());
    }
    Ok(())
}
