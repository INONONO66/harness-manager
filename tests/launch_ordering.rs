use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn unique_suite(label: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("hm-launch-order-{label}-{}-{nanos}", std::process::id())
}

#[test]
fn npm_isolated_harness_does_not_fall_back_to_host_path_binary() {
    let suite = unique_suite("npm-isolated-no-host-fallback");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let tmp_home = std::env::temp_dir().join(format!("{suite}-home"));
    let fake_bin = std::env::temp_dir().join(format!("{suite}-fake-bin"));
    let marker = std::env::temp_dir().join(format!("{suite}-host-omx-ran"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);
    let _ = std::fs::remove_dir_all(&fake_bin);
    let _ = std::fs::remove_file(&marker);
    std::fs::create_dir_all(tmp_cfg.join("hm")).unwrap();
    std::fs::create_dir_all(&tmp_data).unwrap();
    std::fs::create_dir_all(&tmp_home).unwrap();
    std::fs::create_dir_all(&fake_bin).unwrap();
    std::fs::write(
        tmp_cfg.join("hm/config.toml"),
        "default_profile = \"qa\"\n[profiles.qa.gateway]\nbase_url = \"http://127.0.0.1:9/v1\"\nbearer = \"qa-token\"\nproviders = [\"openai\"]\n",
    )
    .unwrap();
    let fake_omx = fake_bin.join("omx");
    std::fs::write(
        &fake_omx,
        format!("#!/bin/sh\nprintf ran > '{}'\nexit 42\n", marker.display()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        let mut permissions = std::fs::metadata(&fake_omx).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&fake_omx, permissions).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "omx", "--profile", "qa", "--", "--sentinel"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env("HOME", &tmp_home)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", fake_bin.display()),
        )
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_ACCESS_TOKEN")
        .output()
        .expect("spawn hm");

    let exit_code = output.status.code();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let marker_exists = marker.exists();

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);
    let _ = std::fs::remove_dir_all(&fake_bin);
    let _ = std::fs::remove_file(&marker);

    assert_ne!(
        exit_code,
        Some(42),
        "hm must not exec host PATH omx for npm-isolated harnesses; stderr was: {stderr}"
    );
    assert!(
        !marker_exists,
        "host PATH omx was executed, so npm-isolated launch escaped isolation"
    );
    assert!(
        stderr.contains("hm harness install omx") || stderr.contains("isolated"),
        "expected install guidance for missing isolated omx; stderr was: {stderr}"
    );
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
fn use_without_profile_and_without_default_profile_launches_profileless() {
    let suite = unique_suite("no-profile-no-default");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let tmp_home = std::env::temp_dir().join(format!("{suite}-home"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);
    std::fs::create_dir_all(tmp_cfg.join("hm")).unwrap();
    std::fs::create_dir_all(&tmp_data).unwrap();
    std::fs::create_dir_all(&tmp_home).unwrap();
    std::fs::write(tmp_cfg.join("hm/config.toml"), "").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "codex", "--print-env"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env("HOME", &tmp_home)
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_ACCESS_TOKEN")
        .output()
        .expect("spawn hm");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);

    assert!(
        output.status.success(),
        "hm use should proceed profile-less when neither --profile nor default_profile is set; stderr was: {stderr}"
    );
    assert!(
        stdout.contains("CODEX_HOME="),
        "expected profile-less codex launch env, got stdout: {stdout}"
    );
    assert!(
        !stderr.contains("no profile specified"),
        "profile-less launch must not call explicit profile resolution: {stderr}"
    );
}

#[test]
fn profile_launch_does_not_share_host_auth_into_harness_isolation() {
    let suite = unique_suite("profile-auth-not-shared");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let tmp_home = std::env::temp_dir().join(format!("{suite}-home"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);
    std::fs::create_dir_all(tmp_cfg.join("hm")).unwrap();
    std::fs::create_dir_all(tmp_home.join(".codex")).unwrap();
    std::fs::write(tmp_home.join(".codex/auth.json"), r#"{"token":"host"}"#).unwrap();
    std::fs::write(
        tmp_cfg.join("hm/config.toml"),
        "default_profile = \"proxy\"\n[profiles.proxy.gateway]\nbase_url = \"http://127.0.0.1:9/v1\"\nbearer = \"qa-token\"\nproviders = [\"openai\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "omx", "--profile", "proxy", "--print-env"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env("HOME", &tmp_home)
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_ACCESS_TOKEN")
        .output()
        .expect("spawn hm");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let isolated_auth = tmp_data.join("hm/runtimes/omx/home/.codex/auth.json");
    let isolated_config = tmp_data.join("hm/runtimes/omx/home/.codex/config.toml");
    let auth_exists = isolated_auth.exists();
    let config_exists = isolated_config.exists();

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);

    assert!(
        output.status.success(),
        "profile launch env assembly should succeed; stderr was: {stderr}"
    );
    assert!(
        config_exists,
        "profile launch should still seed runtime config into isolation"
    );
    assert!(
        !auth_exists,
        "profile launch must not share host auth file into harness isolation"
    );
}

#[test]
fn no_profile_launch_shares_host_auth_when_runtime_manifest_allows_it() {
    let suite = unique_suite("no-profile-auth-shared");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let tmp_home = std::env::temp_dir().join(format!("{suite}-home"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);
    std::fs::create_dir_all(tmp_cfg.join("hm")).unwrap();
    std::fs::create_dir_all(tmp_home.join(".codex")).unwrap();
    std::fs::write(tmp_home.join(".codex/auth.json"), r#"{"token":"host"}"#).unwrap();
    std::fs::write(tmp_cfg.join("hm/config.toml"), "").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "omx", "--print-env"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env("HOME", &tmp_home)
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_ACCESS_TOKEN")
        .output()
        .expect("spawn hm");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let isolated_auth = tmp_data.join("hm/runtimes/omx/home/.codex/auth.json");
    let auth_is_link = std::fs::symlink_metadata(&isolated_auth)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);

    assert!(
        output.status.success(),
        "no-profile launch env assembly should succeed; stderr was: {stderr}"
    );
    assert!(
        auth_is_link,
        "no-profile launch should keep runtime-manifest host auth sharing"
    );
}

#[test]
fn profile_launch_removes_stale_host_auth_link_from_previous_no_profile_launch() {
    let suite = unique_suite("profile-removes-shared-auth");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let tmp_home = std::env::temp_dir().join(format!("{suite}-home"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);
    std::fs::create_dir_all(tmp_cfg.join("hm")).unwrap();
    std::fs::create_dir_all(tmp_home.join(".codex/sessions")).unwrap();
    std::fs::write(tmp_home.join(".codex/auth.json"), r#"{"token":"host"}"#).unwrap();
    std::fs::write(tmp_home.join(".codex/sessions/events.sqlite"), "db").unwrap();
    std::fs::write(tmp_cfg.join("hm/config.toml"), "").unwrap();

    let first = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "omx", "--print-env"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env("HOME", &tmp_home)
        .output()
        .expect("spawn hm");

    let auth = tmp_data.join("hm/runtimes/omx/home/.codex/auth.json");
    let db = tmp_data.join("hm/runtimes/omx/home/.codex/sessions/events.sqlite");
    let auth_was_link = std::fs::symlink_metadata(&auth)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
    let db_was_link = std::fs::symlink_metadata(&db)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);

    std::fs::write(
        tmp_cfg.join("hm/config.toml"),
        "default_profile = \"proxy\"\n[profiles.proxy.gateway]\nbase_url = \"http://127.0.0.1:9/v1\"\nbearer = \"qa-token\"\nproviders = [\"openai\"]\n",
    )
    .unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["use", "omx", "--profile", "proxy", "--print-env"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env("HOME", &tmp_home)
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("CODEX_API_KEY")
        .env_remove("CODEX_ACCESS_TOKEN")
        .output()
        .expect("spawn hm");

    let auth_exists_after_profile = auth.exists();
    let db_still_link = std::fs::symlink_metadata(&db)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);

    assert!(first.status.success(), "initial no-profile launch failed");
    assert!(auth_was_link, "initial no-profile launch should share auth");
    assert!(
        db_was_link,
        "initial no-profile launch should share DB files"
    );
    assert!(second.status.success(), "profile launch failed");
    assert!(
        !auth_exists_after_profile,
        "profile launch must remove stale host auth link"
    );
    assert!(
        db_still_link,
        "profile launch should keep runtime database sharing"
    );
}

#[test]
fn missing_explicit_xdg_config_defaults_instead_of_reading_missing_file() {
    let suite = unique_suite("missing-explicit-xdg-config");
    let tmp_cfg = std::env::temp_dir().join(format!("{suite}-cfg"));
    let tmp_data = std::env::temp_dir().join(format!("{suite}-data"));
    let tmp_home = std::env::temp_dir().join(format!("{suite}-home"));
    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);
    std::fs::create_dir_all(&tmp_cfg).unwrap();
    std::fs::create_dir_all(&tmp_data).unwrap();
    std::fs::create_dir_all(&tmp_home).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["inject", "plan", "codex", "--profile", "missing"])
        .env("XDG_CONFIG_HOME", &tmp_cfg)
        .env("XDG_DATA_HOME", &tmp_data)
        .env("HOME", &tmp_home)
        .output()
        .expect("spawn hm");

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let _ = std::fs::remove_dir_all(&tmp_cfg);
    let _ = std::fs::remove_dir_all(&tmp_data);
    let _ = std::fs::remove_dir_all(&tmp_home);

    assert!(
        !output.status.success(),
        "missing profile should fail against default config"
    );
    assert!(
        stderr.contains("profile 'missing' not found in config"),
        "expected default-config profile error, got: {stderr}"
    );
    assert!(
        !stderr.contains("failed to read config"),
        "missing explicit XDG config must not be treated as unreadable: {stderr}"
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
