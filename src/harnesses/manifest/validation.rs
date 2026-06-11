use anyhow::{bail, Context, Result};

pub(super) fn parse_mode(path_label: &str, mode: Option<&str>) -> Result<Option<u32>> {
    let Some(mode) = mode else {
        return Ok(None);
    };
    let trimmed = mode.strip_prefix('0').unwrap_or(mode);
    let parsed = u32::from_str_radix(trimmed, 8)
        .with_context(|| format!("{path_label}: seed_files.mode must be octal"))?;
    Ok(Some(parsed))
}

pub(super) fn ensure(condition: bool, path_label: &str, field: &str) -> Result<()> {
    if condition {
        return Ok(());
    }
    bail!("{path_label}: invalid {field}")
}

pub(crate) fn validate_id(path_label: &str, id: &str) -> Result<()> {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        bail!("{path_label}: invalid id");
    };
    ensure(first.is_ascii_lowercase(), path_label, "id")?;
    ensure((2..=64).contains(&id.len()), path_label, "id")?;
    ensure(
        id.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'),
        path_label,
        "id",
    )
}

pub(crate) fn validate_binary_name(path_label: &str, field: &str, value: &str) -> Result<()> {
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

pub(super) fn validate_env_key(path_label: &str, key: &str) -> Result<()> {
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

pub(crate) fn validate_package_name(path_label: &str, field: &str, value: &str) -> Result<()> {
    validate_package_common(path_label, field, value)?;
    let valid = if let Some((scope, name)) = value.strip_prefix('@').and_then(|v| v.split_once('/'))
    {
        !scope.is_empty() && valid_npm_part(scope) && !name.is_empty() && valid_npm_part(name)
    } else {
        !value.contains('/') && valid_npm_part(value)
    };
    ensure(valid, path_label, field)
}

pub(crate) fn validate_python_package_name(
    path_label: &str,
    field: &str,
    value: &str,
) -> Result<()> {
    validate_package_common(path_label, field, value)?;
    let (package, extras) = match value.split_once('[') {
        Some((package, extras_part)) => {
            ensure(extras_part.ends_with(']'), path_label, field)?;
            ensure(
                !extras_part[..extras_part.len() - 1].contains('['),
                path_label,
                field,
            )?;
            (package, Some(&extras_part[..extras_part.len() - 1]))
        }
        None => (value, None),
    };
    ensure(!package.is_empty(), path_label, field)?;
    ensure(
        package
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
        path_label,
        field,
    )?;
    if let Some(extras) = extras {
        ensure(!extras.is_empty(), path_label, field)?;
        for extra in extras.split(',') {
            ensure(!extra.is_empty(), path_label, field)?;
            ensure(
                extra
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')),
                path_label,
                field,
            )?;
        }
    }
    Ok(())
}

fn validate_package_common(path_label: &str, field: &str, value: &str) -> Result<()> {
    ensure(!value.is_empty(), path_label, field)?;
    ensure(!has_control(value), path_label, field)?;
    ensure(!value.starts_with('-'), path_label, field)?;
    ensure(
        !value.contains("://") && !value.starts_with("git+"),
        path_label,
        field,
    )?;
    ensure(
        !value.starts_with('.') && !value.starts_with('/') && !value.starts_with('~'),
        path_label,
        field,
    )
}

pub(super) fn validate_args(path_label: &str, field: &str, args: &[String]) -> Result<()> {
    for arg in args {
        ensure(!has_control(arg), path_label, field)?;
    }
    Ok(())
}

fn valid_npm_part(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
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
