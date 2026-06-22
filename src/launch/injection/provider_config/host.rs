use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{Map, Value};

pub(super) fn merge_host_provider_config(
    target: &mut Value,
    root_key: &str,
    config_path: &Path,
    isolation_home: &Path,
    bearer: &str,
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
    merge_missing_provider_entries(target, root_key, &host, bearer);
    resolve_host_secret_refs_in_place(target, bearer);
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

fn merge_missing_provider_entries(target: &mut Value, root_key: &str, host: &Value, bearer: &str) {
    let Some(host_providers) = value_at_path(host, root_key).and_then(Value::as_object) else {
        return;
    };
    let target_providers = object_at_path(target, root_key);
    for (provider, config) in host_providers {
        target_providers
            .entry(provider.clone())
            .or_insert_with(|| resolve_host_secret_refs(config.clone(), bearer));
    }
}

fn resolve_host_secret_refs(mut value: Value, bearer: &str) -> Value {
    resolve_host_secret_refs_in_place(&mut value, bearer);
    value
}

fn resolve_host_secret_refs_in_place(value: &mut Value, bearer: &str) {
    match value {
        Value::String(text) => {
            if is_env_ref(text) {
                *text = bearer.to_string();
            } else if let Some(rest) = text.strip_prefix("Bearer ") {
                if is_env_ref(rest) {
                    *text = format!("Bearer {bearer}");
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                resolve_host_secret_refs_in_place(value, bearer);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                resolve_host_secret_refs_in_place(value, bearer);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn is_env_ref(value: &str) -> bool {
    (value.starts_with("{env:") || value.starts_with("{file:")) && value.ends_with('}')
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
    use super::{merge_missing_provider_entries, resolve_host_secret_refs_in_place};
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

        merge_missing_provider_entries(&mut target, "provider", &host, "profile-key");

        assert_eq!(
            target["provider"]["anthropic"]["options"]["baseURL"].as_str(),
            Some("https://profile.example/v1")
        );
        assert_eq!(
            target["provider"]["zai-coding-plan"]["options"]["baseURL"].as_str(),
            Some("https://proxy.example/v1")
        );
        assert_eq!(
            target["provider"]["zai-coding-plan"]["options"]["apiKey"].as_str(),
            Some("host-key")
        );
    }

    #[test]
    fn host_provider_merge_resolves_env_placeholders_for_imported_custom_providers() {
        let mut target = json!({ "provider": {} });
        let host = json!({
            "provider": {
                "zai-coding-plan": {
                    "options": {
                        "baseURL": "https://proxy.example/v1",
                        "apiKey": "{file:/Users/example/.config/agent-proxy/macbook.key}",
                        "headers": {
                            "Authorization": "Bearer {file:/Users/example/.config/agent-proxy/macbook.key}",
                            "x-api-key": "{file:/Users/example/.config/agent-proxy/macbook.key}"
                        }
                    }
                }
            }
        });

        merge_missing_provider_entries(&mut target, "provider", &host, "profile-key");

        let options = &target["provider"]["zai-coding-plan"]["options"];
        assert_eq!(options["apiKey"].as_str(), Some("profile-key"));
        assert_eq!(
            options["headers"]["Authorization"].as_str(),
            Some("Bearer profile-key")
        );
        assert_eq!(
            options["headers"]["x-api-key"].as_str(),
            Some("profile-key")
        );
    }

    #[test]
    fn host_provider_merge_resolves_placeholders_already_in_target_config() {
        let mut target = json!({
            "provider": {
                "zai-coding-plan": {
                    "options": {
                        "baseURL": "https://proxy.example/v1",
                        "apiKey": "{file:/Users/example/.config/agent-proxy/macbook.key}",
                        "headers": {
                            "Authorization": "Bearer {file:/Users/example/.config/agent-proxy/macbook.key}"
                        }
                    }
                }
            }
        });
        let host = json!({ "provider": {} });

        merge_missing_provider_entries(&mut target, "provider", &host, "profile-key");
        resolve_host_secret_refs_in_place(&mut target, "profile-key");

        let options = &target["provider"]["zai-coding-plan"]["options"];
        assert_eq!(options["apiKey"].as_str(), Some("profile-key"));
        assert_eq!(
            options["headers"]["Authorization"].as_str(),
            Some("Bearer profile-key")
        );
    }
}
