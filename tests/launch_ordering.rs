use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_suite(label: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("hm-launch-order-{label}-{}-{nanos}", std::process::id())
}

#[test]
fn broken_default_profile_fails_without_creating_isolation_directories() {
    let suite = unique_suite("broken-default");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    std::fs::create_dir_all(tmp_cfg.join("hm")).unwrap();
    std::fs::create_dir_all(&tmp_data).unwrap();
    std::fs::write(
        tmp_cfg.join("hm/config.toml"),
        "default_profile = \"nonexistent-profile\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "claude", "--print-env"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("ANTHROPIC_BASE_URL")
        .output()
        .expect("spawn hm");

    let exit_code = output.status.code();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let hm_data_root = tmp_data.join("hm");
    let hm_data_root_exists = hm_data_root.exists();
    let leftover_entries: Vec<String> = if hm_data_root_exists {
        std::fs::read_dir(&hm_data_root)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path().display().to_string())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);

    assert_ne!(
        exit_code,
        Some(0),
        "hm use must fail when default_profile points to a nonexistent profile; stderr was: {stderr}"
    );
    assert!(
        stderr.contains("nonexistent-profile") && stderr.contains("not found"),
        "expected 'profile not found' error in stderr; got: {stderr}"
    );
    assert!(
        !hm_data_root_exists,
        "ordering contract violated: hm created {} after profile resolution failed. Leftover entries: {:?}",
        hm_data_root.display(),
        leftover_entries
    );
}

#[test]
fn config_parse_failure_fails_without_creating_isolation_directories() {
    let suite = unique_suite("bad-config-syntax");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    std::fs::create_dir_all(tmp_cfg.join("hm")).unwrap();
    std::fs::create_dir_all(&tmp_data).unwrap();
    std::fs::write(
        tmp_cfg.join("hm/config.toml"),
        "default_profile = unterminated\n[profiles.broken\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "claude", "--print-env"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("ANTHROPIC_BASE_URL")
        .output()
        .expect("spawn hm");

    let exit_code = output.status.code();
    let hm_data_root_exists = tmp_data.join("hm").exists();

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);

    assert_ne!(exit_code, Some(0), "parse failure must exit non-zero");
    assert!(
        !hm_data_root_exists,
        "parse failure must not create $XDG_DATA_HOME/hm/"
    );
}
