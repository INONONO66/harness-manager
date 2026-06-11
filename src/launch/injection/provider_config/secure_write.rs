use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

pub(super) fn write_provider_config_file(config_path: &Path, contents: &str) -> Result<PathBuf> {
    write_provider_config_file_before_rename(config_path, contents, |_| Ok(()))
}

fn write_provider_config_file_before_rename(
    config_path: &Path,
    contents: &str,
    before_rename: impl FnOnce(&Path) -> Result<()>,
) -> Result<PathBuf> {
    let tmp_path = temp_path(config_path)?;
    let result = (|| -> Result<()> {
        let mut open_options = OpenOptions::new();
        open_options.write(true).create_new(true);
        set_owner_only_create_mode(&mut open_options);
        let mut tmp = open_options
            .open(&tmp_path)
            .with_context(|| format!("failed to create {}", tmp_path.display()))?;
        set_owner_only_permissions(&tmp)?;
        tmp.write_all(contents.as_bytes())
            .with_context(|| format!("failed to write {}", tmp_path.display()))?;
        tmp.sync_all()
            .with_context(|| format!("failed to sync {}", tmp_path.display()))?;
        drop(tmp);
        before_rename(&tmp_path)?;
        fs::rename(&tmp_path, config_path).with_context(|| {
            format!(
                "failed to replace {} with {}",
                config_path.display(),
                tmp_path.display()
            )
        })?;
        set_owner_only_permissions_path(config_path)?;
        Ok(())
    })();
    let _ = fs::remove_file(&tmp_path);
    result?;
    Ok(config_path.to_path_buf())
}

fn temp_path(config_path: &Path) -> Result<PathBuf> {
    let file_name = config_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("provider config path has no file name"))?
        .to_string_lossy();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_nanos();
    Ok(config_path.with_file_name(format!(".{file_name}.hm-{nanos}.tmp")))
}

#[cfg(unix)]
fn set_owner_only_create_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_owner_only_create_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn set_owner_only_permissions(file: &fs::File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = file
        .metadata()
        .context("failed to read provider config temp file metadata")?
        .permissions();
    permissions.set_mode(0o600);
    file.set_permissions(permissions)
        .context("failed to restrict provider config temp file permissions")
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_file: &fs::File) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_permissions_path(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to restrict permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_owner_only_permissions_path(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_write_before_rename_preserves_existing_config() {
        // Given: a provider config already exists and a replacement fails before rename.
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("provider.json");
        let original = "{ \"provider\": { \"openai\": \"old\" } }\n";
        fs::write(&target, original).unwrap();

        // When: the secure writer reports an error after writing the temp file.
        let err =
            write_provider_config_file_before_rename(&target, "{ \"provider\": {} }\n", |_| {
                anyhow::bail!("simulated write failure before rename")
            })
            .unwrap_err();

        // Then: the original config is still intact and the temp file is cleaned up.
        assert!(
            err.to_string().contains("simulated write failure"),
            "expected simulated failure: {err:#}"
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), original);
        let leftovers: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".provider.json.hm-")
            })
            .collect();
        assert!(leftovers.is_empty(), "leftover temp files: {leftovers:?}");
    }
}
