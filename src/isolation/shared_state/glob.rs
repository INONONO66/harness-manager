use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::link::reject_source_symlink_chain;

pub(super) fn expand_one_segment_glob(
    main_home: &Path,
    pattern: &str,
    label: &str,
) -> Result<Vec<PathBuf>> {
    if pattern.contains("**") || pattern.contains('\\') {
        anyhow::bail!("{label} contains an unsafe glob");
    }
    let components: Vec<&str> = pattern.split('/').collect();
    if components
        .iter()
        .any(|component| component.is_empty() || *component == "." || *component == "..")
    {
        anyhow::bail!("{label} contains an unsafe path component");
    }
    let wildcard_count = components
        .iter()
        .filter(|component| component.contains('*'))
        .count();
    if wildcard_count > 1 {
        anyhow::bail!("{label} may contain a wildcard in only one path segment");
    }
    expand_glob_components(main_home, &components, PathBuf::new(), label)
}

fn expand_glob_components(
    main_home: &Path,
    components: &[&str],
    relative_prefix: PathBuf,
    label: &str,
) -> Result<Vec<PathBuf>> {
    let Some((component, rest)) = components.split_first() else {
        return Ok(vec![relative_prefix]);
    };
    if !component.contains('*') {
        return expand_glob_components(main_home, rest, relative_prefix.join(component), label);
    }
    let source_dir = main_home.join(&relative_prefix);
    reject_source_symlink_chain(main_home, &source_dir, label)?;
    if !source_dir.exists() {
        return Ok(Vec::new());
    }
    let mut matches = Vec::new();
    for entry in
        fs::read_dir(&source_dir).with_context(|| format!("read {}", source_dir.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !glob_segment_matches(component, name) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .with_context(|| format!("inspect {}", entry.path().display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        matches.extend(expand_glob_components(
            main_home,
            rest,
            relative_prefix.join(name),
            label,
        )?);
    }
    Ok(matches)
}

pub(super) fn glob_segment_matches(pattern: &str, value: &str) -> bool {
    let mut remainder = value;
    let mut first = true;
    for part in pattern.split('*') {
        if part.is_empty() {
            continue;
        }
        if first && !pattern.starts_with('*') {
            let Some(stripped) = remainder.strip_prefix(part) else {
                return false;
            };
            remainder = stripped;
        } else {
            let Some(index) = remainder.find(part) else {
                return false;
            };
            remainder = &remainder[index + part.len()..];
        }
        first = false;
    }
    pattern.ends_with('*') || remainder.is_empty()
}
