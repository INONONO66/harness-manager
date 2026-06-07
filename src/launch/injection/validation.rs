use anyhow::Result;

pub(super) fn validate_header_name_at_runtime(name: &str) -> Result<()> {
    if name.is_empty()
        || name
            .bytes()
            .any(|b| !b.is_ascii_graphic() || b == b':' || b.is_ascii_control())
    {
        anyhow::bail!(
            "gateway provider_headers: invalid header name '{}' (must be printable ASCII, no ':' or control chars)",
            name
        );
    }
    Ok(())
}

pub(super) fn validate_bearer_value_at_runtime(value: &str) -> Result<()> {
    if value.trim().is_empty() {
        anyhow::bail!(
            "bearer is empty or whitespace-only (set a real credential in [profiles.<name>.gateway].bearer or [profiles.<name>.llm].bearer)"
        );
    }
    if value
        .chars()
        .any(|ch| ch == '\r' || ch == '\n' || ch == '\0')
    {
        anyhow::bail!("bearer contains CRLF/NUL (refused to prevent header injection)");
    }
    if value.chars().any(char::is_control) {
        anyhow::bail!("bearer contains control characters");
    }
    Ok(())
}

fn validate_header_value_at_runtime(name: &str, value: &str) -> Result<()> {
    if value
        .chars()
        .any(|ch| ch == '\r' || ch == '\n' || ch == '\0')
    {
        anyhow::bail!(
            "gateway provider_headers: header '{}' value contains CRLF/NUL (refused to prevent header injection)",
            name
        );
    }
    if value.chars().any(char::is_control) {
        anyhow::bail!(
            "gateway provider_headers: header '{}' value contains control characters",
            name
        );
    }
    Ok(())
}

pub(super) fn render_header_value_at_runtime(
    name: &str,
    template: &str,
    bearer: &str,
) -> Result<String> {
    validate_header_value_at_runtime(name, template)?;
    let value = template.replace("{bearer}", bearer);
    validate_header_value_at_runtime(name, &value)?;
    Ok(value)
}

pub(super) fn effective_endpoint(base_url: &str, strip_v1: bool) -> String {
    if !strip_v1 {
        return base_url.to_string();
    }
    base_url
        .trim_end_matches('/')
        .trim_end_matches("/v1")
        .to_string()
}
