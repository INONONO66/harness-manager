use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

pub(super) fn merge_host_provider_config(
    target: &mut Value,
    root_key: &str,
    config_path: &Path,
    isolation_home: &Path,
) -> Result<()> {
    let Some(host_path) = host_config_path(config_path, isolation_home) else {
        return Ok(());
    };
    if !host_path.is_file() {
        return Ok(());
    }
    let existing = fs::read_to_string(&host_path).with_context(|| {
        format!(
            "failed to read host provider config {}",
            host_path.display()
        )
    })?;
    let host: Value = serde_json::from_str(&existing).with_context(|| {
        format!(
            "failed to parse host provider config {} while importing missing providers",
            host_path.display()
        )
    })?;
    merge_missing_provider_entries(target, root_key, &host);
    Ok(())
}

fn host_config_path(config_path: &Path, isolation_home: &Path) -> Option<PathBuf> {
    if !is_hm_runtime_home(isolation_home) {
        return None;
    }
    let relative = config_path.strip_prefix(isolation_home).ok()?;
    let home = dirs::home_dir()?;
    let host_path = home.join(relative);
    (host_path != config_path).then_some(host_path)
}

fn is_hm_runtime_home(path: &Path) -> bool {
    let mut components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str());
    while let Some(component) = components.next() {
        if component == "hm"
            && components.next() == Some("runtimes")
            && components.next().is_some()
            && components.next() == Some("home")
        {
            return true;
        }
    }
    false
}

fn merge_missing_provider_entries(target: &mut Value, root_key: &str, host: &Value) {
    let Some(host_providers) = value_at_path(host, root_key).and_then(Value::as_object) else {
        return;
    };
    let target_providers = object_at_path(target, root_key);
    for (provider, config) in host_providers {
        target_providers
            .entry(provider.clone())
            .or_insert_with(|| config.clone());
    }
}

fn value_at_path<'a>(value: &'a Value, dotted_path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in dotted_path.split('.') {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

fn object_at_path<'a>(value: &'a mut Value, dotted_path: &str) -> &'a mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    let mut current = value;
    for part in dotted_path.split('.') {
        let object = current
            .as_object_mut()
            .expect("current provider config path segment is an object");
        current = object
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
    }
    current
        .as_object_mut()
        .expect("provider config root is an object")
}

#[cfg(test)]
mod tests {
    use super::merge_missing_provider_entries;
    use serde_json::json;

    #[test]
    fn host_provider_merge_preserves_custom_providers_without_overwriting_profile_providers() {
        let mut target = json!({
            "provider": {
                "anthropic": {
                    "options": {
                        "baseURL": "https://profile.example/v1",
                        "apiKey": "profile-key"
                    }
                }
            }
        });
        let host = json!({
            "provider": {
                "anthropic": {
                    "options": {
                        "baseURL": "https://host.example/v1",
                        "apiKey": "host-key"
                    }
                },
                "zai-coding-plan": {
                    "options": {
                        "baseURL": "https://proxy.example/v1",
                        "apiKey": "host-key"
                    }
                }
            }
        });

        merge_missing_provider_entries(&mut target, "provider", &host);

        assert_eq!(
            target["provider"]["anthropic"]["options"]["baseURL"].as_str(),
            Some("https://profile.example/v1")
        );
        assert_eq!(
            target["provider"]["zai-coding-plan"]["options"]["baseURL"].as_str(),
            Some("https://proxy.example/v1")
        );
    }
}
