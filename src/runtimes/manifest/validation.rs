use anyhow::{bail, Context, Result};

pub(super) fn ensure(condition: bool, path_label: &str, field: &str) -> Result<()> {
    if condition {
        return Ok(());
    }
    bail!("{path_label}: invalid {field}")
}

pub(super) fn parse_mode(path_label: &str, mode: Option<&str>) -> Result<Option<u32>> {
    let Some(mode) = mode else {
        return Ok(None);
    };
    let trimmed = mode.strip_prefix('0').unwrap_or(mode);
    let parsed = u32::from_str_radix(trimmed, 8)
        .with_context(|| format!("{path_label}: seed_files.mode must be octal"))?;
    Ok(Some(parsed))
}

pub(super) fn validate_display_name(path_label: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, "name")?;
    ensure(!has_control(value), path_label, "name")?;
    ensure((1..=64).contains(&value.len()), path_label, "name")
}

pub(super) fn validate_binary_name(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure(!has_control(value), path_label, field)?;
    ensure(!value.contains(['/', '\\']), path_label, field)?;
    ensure(!value.contains('.'), path_label, field)
}

pub(super) fn validate_relative_path(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure(!has_control(value), path_label, field)?;
    ensure(
        !value.starts_with('/') && !value.starts_with('~'),
        path_label,
        field,
    )?;
    ensure(!value.contains('\\'), path_label, field)?;
    ensure(
        !value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == ".."),
        path_label,
        field,
    )
}

pub(super) fn validate_seed_path(path_label: &str, value: &str) -> Result<()> {
    ensure(
        value.starts_with("{home}/")
            || value.starts_with("{runtime_home}/")
            || value.starts_with("{state}/")
            || value.starts_with("{tmp}/"),
        path_label,
        "isolation.seed_files",
    )?;
    ensure(
        !value.starts_with("../"),
        path_label,
        "isolation.seed_files",
    )?;
    ensure(!value.contains("/../"), path_label, "isolation.seed_files")
}

pub(super) fn validate_static_env_key(path_label: &str, key: &str) -> Result<()> {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        bail!("{path_label}: invalid isolation.static_envs");
    };
    let shape_ok = (first.is_ascii_uppercase() || first == '_')
        && chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_');
    let denied = matches!(
        key,
        "HOME" | "PATH" | "SHELL" | "PWD" | "_" | "SSH_AUTH_SOCK"
    ) || key.starts_with("LD_")
        || key.starts_with("DYLD_")
        || key.ends_with("_TOKEN")
        || key.ends_with("_SECRET")
        || key.ends_with("_API_KEY");
    if shape_ok && !denied {
        return Ok(());
    }
    bail!("{path_label}: invalid isolation.static_envs.{key}")
}

pub(super) fn validate_env_name_shape(path_label: &str, field: &str, key: &str) -> Result<()> {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        bail!("{path_label}: invalid {field}");
    };
    let ok = (first.is_ascii_uppercase() || first == '_')
        && chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_');
    ensure(ok, path_label, field)
}

pub(super) fn validate_template_value(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!has_control(value), path_label, field)?;
    ensure(
        !value.starts_with('/') && !value.starts_with('~'),
        path_label,
        field,
    )?;
    ensure(
        !value.contains("../") && !value.contains("/.."),
        path_label,
        field,
    )?;
    for token in tokens(value) {
        ensure(
            matches!(
                token,
                "home" | "state" | "tmp" | "runtime_home" | "runtime_state" | "runtime_logs"
            ),
            path_label,
            field,
        )?;
    }
    Ok(())
}

pub(super) fn validate_args(path_label: &str, field: &str, args: &[String]) -> Result<()> {
    for arg in args {
        ensure(!has_control(arg), path_label, field)?;
    }
    Ok(())
}

pub(super) fn validate_provider_name(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure((1..=64).contains(&value.len()), path_label, field)?;
    ensure(
        value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'),
        path_label,
        field,
    )
}

pub(super) fn validate_header_name(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure(
        value
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b':' && !b.is_ascii_control()),
        path_label,
        field,
    )
}

pub(super) fn validate_header_value_template(
    path_label: &str,
    field: &str,
    value: &str,
) -> Result<()> {
    ensure(
        !value
            .chars()
            .any(|ch| ch == '\r' || ch == '\n' || ch == '\0'),
        path_label,
        field,
    )?;
    ensure(!value.chars().any(char::is_control), path_label, field)?;
    Ok(())
}

pub(super) fn validate_config_path(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure(!has_control(value), path_label, field)?;
    ensure(value.starts_with("{home}/"), path_label, field)?;
    ensure(!value.starts_with("../"), path_label, field)?;
    ensure(!value.contains("/../"), path_label, field)?;
    Ok(())
}

pub(super) fn validate_dotted_key(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure(!has_control(value), path_label, field)?;
    for segment in value.split('.') {
        ensure(!segment.is_empty(), path_label, field)?;
        ensure(
            segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
            path_label,
            field,
        )?;
    }
    Ok(())
}

/// Validates a TOML bare key: ASCII alphanumeric, underscore, or hyphen ONLY.
/// Rejects dots, brackets, quotes, equals, whitespace, and control chars.
/// This is the strict inverse of `validate_dotted_key`: a bare key cannot
/// contain `.`. Used by `codex-config-seed` for top-level key names that
/// must be written via `toml_edit::DocumentMut[key] = value(...)`.
pub(super) fn validate_toml_bare_key(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure(!has_control(value), path_label, field)?;
    ensure(
        value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        path_label,
        field,
    )?;
    Ok(())
}

fn has_control(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn tokens(value: &str) -> impl Iterator<Item = &str> {
    value.match_indices('{').filter_map(|(start, _)| {
        value[start + 1..]
            .find('}')
            .map(|end| &value[start + 1..start + 1 + end])
    })
}
