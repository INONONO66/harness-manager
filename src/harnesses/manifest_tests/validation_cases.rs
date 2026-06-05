use super::{minimal_manifest, parse_toml};

#[test]
fn manifest_rejects_unknown_package_kind() {
    // Given: a manifest with an unsupported package kind.
    let input = minimal_manifest("").replace(r#"kind = "npm-global""#, r#"kind = "shell""#);

    // When: the manifest is parsed.
    let err = parse_toml("demo.toml", &input).expect_err("package kind must fail");

    // Then: the error points at the package discriminator.
    assert!(
        err.to_string().contains("package.kind"),
        "error should mention package.kind, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_bad_seed_path_before_side_effects() {
    // Given: a manifest that attempts to seed outside isolation paths.
    let temp = tempfile::tempdir().expect("tempdir");
    let runtime_dir = temp.path().join("hm").join("runtimes").join("demo");
    let input = minimal_manifest(
        r#"
[[isolation.seed_files]]
path = "../host.toml"
content = "unsafe"
overwrite = true
"#,
    );

    // When: the manifest is parsed.
    let err = parse_toml("demo.toml", &input).expect_err("bad seed path must fail");

    // Then: conversion rejects the path without creating isolation state.
    assert!(
        err.to_string().contains("seed_files"),
        "error should mention seed_files, got: {err:#}"
    );
    assert!(
        !runtime_dir.exists(),
        "manifest parsing must not create isolation directories"
    );
}

#[test]
fn manifest_rejects_bare_seed_path_before_side_effects() {
    // Given: a manifest that writes to an unrooted path.
    let input = minimal_manifest(
        r#"
[[isolation.seed_files]]
path = "config.toml"
content = "unsafe"
overwrite = true
"#,
    );

    // When: the manifest is parsed.
    let err = parse_toml("demo.toml", &input).expect_err("bare seed path must fail");

    // Then: conversion rejects the path at the manifest boundary.
    assert!(
        err.to_string().contains("seed_files"),
        "error should mention seed_files, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_runtime_binary_id_collision() {
    // Given: a manifest id that would shadow the Codex runtime command.
    let input = minimal_manifest("").replace(r#"id = "demo""#, r#"id = "codex""#);

    // When: the manifest is parsed.
    let err = parse_toml("shadow.toml", &input).expect_err("runtime collision must fail");

    // Then: conversion rejects the ambiguous id before command routing.
    assert!(
        err.to_string().contains("id"),
        "error should mention id, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_empty_detect_binaries() {
    // Given: a manifest with no detection binaries.
    let input =
        minimal_manifest("").replace(r#"detect_binaries = ["demo"]"#, "detect_binaries = []");

    // When: the manifest is parsed.
    let err = parse_toml("demo.toml", &input).expect_err("empty detect_binaries must fail");

    // Then: the error names the invalid field.
    assert!(
        err.to_string().contains("detect_binaries"),
        "error should mention detect_binaries, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_package_option_injection() {
    // Given: a manifest whose package name starts with an option marker.
    let input = minimal_manifest("").replace(
        r#"package = "demo-package""#,
        r#"package = "--prefix=/tmp/x""#,
    );

    // When: the manifest is parsed.
    let err = parse_toml("demo.toml", &input).expect_err("package option must fail");

    // Then: the package grammar rejects it.
    assert!(
        err.to_string().contains("package"),
        "error should mention package, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_unknown_top_level_field() {
    // Given: a manifest containing a misspelled top-level field.
    let input = minimal_manifest("unexpected = true");

    // When: the manifest is parsed.
    let err = parse_toml("demo.toml", &input).expect_err("unknown field must fail");

    // Then: serde rejects the unknown field.
    assert!(
        err.to_string().contains("unknown field"),
        "error should mention unknown field, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_bad_static_env_value() {
    // Given: a static env value with parent traversal.
    let input = minimal_manifest("").replace(
        r#"static_envs = { CODEX_HOME = "{home}/.codex" }"#,
        r#"static_envs = { CODEX_HOME = "{home}/../escape" }"#,
    );

    // When: the manifest is parsed.
    let err = parse_toml("demo.toml", &input).expect_err("bad env value must fail");

    // Then: the static env value is rejected at the manifest boundary.
    assert!(
        err.to_string().contains("static_envs"),
        "error should mention static_envs, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_nul_in_args() {
    // Given: a manifest with a NUL in package-manager args.
    let input = minimal_manifest("").replace(
        r#"package = "demo-package""#,
        "package = \"demo-package\"\nargs = [\"run\\u0000now\"]",
    );
    let input = input.replace(r#"kind = "npm-global""#, r#"kind = "npx-installer""#);

    // When: the manifest is parsed.
    let err = parse_toml("nul-args.toml", &input).expect_err("NUL arg must fail");

    // Then: the package args validator rejects the control character.
    assert!(
        err.to_string().contains("package.args"),
        "error should mention package.args, got: {err:#}"
    );
}

