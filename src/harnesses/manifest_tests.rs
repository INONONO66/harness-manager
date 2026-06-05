use super::{parse_toml, ManifestPackageSpec};
use std::path::Path;

fn minimal_manifest(extra: &str) -> String {
    format!(
        r#"
schema_version = 1
id = "demo"
display_name = "Demo Harness"
target_runtime = "Codex CLI"
detect_binaries = ["demo"]

[package]
kind = "npm-global"
package = "demo-package"

[isolation]
spoof_home = true
home_subdirs = [".codex"]
static_envs = {{ CODEX_HOME = "{{home}}/.codex" }}
{extra}
"#
    )
}

#[test]
fn parses_minimal_manifest_into_owned_spec() {
    // Given: a minimal valid manifest.
    let input = minimal_manifest("");

    // When: the manifest is parsed.
    let parsed = parse_toml("demo.toml", &input).expect("manifest parses");

    // Then: owned fields and isolation defaults are preserved.
    assert_eq!(parsed.id, "demo");
    assert_eq!(parsed.display_name, "Demo Harness");
    assert_eq!(parsed.target_runtime, "Codex CLI");
    assert_eq!(parsed.detect_binaries, vec!["demo"]);
    assert_eq!(parsed.launch_binary, None);
    assert!(parsed.launch_args.is_empty());
    assert_eq!(parsed.isolation.subdir, "demo");
    assert_eq!(parsed.isolation.home_subdirs, vec![".codex"]);
    assert_eq!(
        parsed.isolation.static_envs,
        vec![("CODEX_HOME".to_string(), "{home}/.codex".to_string())]
    );
    assert!(parsed.isolation.seed_files.is_empty());
    match parsed.package {
        ManifestPackageSpec::NpmGlobal { package } => assert_eq!(package, "demo-package"),
        other => panic!("unexpected package variant: {other:?}"),
    }
}

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
fn manifest_parses_default_launch_args() {
    // Given: one manifest with omitted launch args and one with fixed launch args.
    let omitted = minimal_manifest("");
    let with_args = minimal_manifest("").replace(
        r#"detect_binaries = ["demo"]"#,
        "detect_binaries = [\"demo\"]\nlaunch_args = [\"run\", \"--fast\"]",
    );

    // When: both manifests are parsed.
    let defaulted = parse_toml("default.toml", &omitted).expect("default parses");
    let explicit = parse_toml("explicit.toml", &with_args).expect("explicit parses");

    // Then: omitted launch args default to empty and explicit args keep order.
    assert!(defaulted.launch_args.is_empty());
    assert_eq!(explicit.launch_args, vec!["run", "--fast"]);
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

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("harnesses")
        .join(name);
    std::fs::read_to_string(path).expect("fixture exists")
}

fn first_toml_block(input: &str) -> &str {
    let start = input.find("```toml").expect("toml block starts") + "```toml".len();
    let rest = &input[start..];
    let end = rest.find("```").expect("toml block ends");
    rest[..end].trim()
}

#[test]
fn documented_demo_manifest_parses() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("harness-manifest.md");
    let docs = std::fs::read_to_string(path).expect("manifest docs exist");
    let demo = first_toml_block(&docs);

    let parsed = parse_toml("docs/harness-manifest.md", demo).expect("documented demo parses");

    assert_eq!(parsed.id, "demo");
}

#[test]
fn manifest_rejects_path_launch_binary() {
    let err = parse_toml(
        "tests/fixtures/harnesses/bad-launch.toml",
        &fixture("bad-launch.toml"),
    )
    .expect_err("path launch binary must fail");

    assert!(
        err.to_string().contains("launch_binary"),
        "error should mention launch_binary, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_secret_static_env() {
    let err = parse_toml(
        "tests/fixtures/harnesses/bad-env.toml",
        &fixture("bad-env.toml"),
    )
    .expect_err("secret static env must fail");

    assert!(
        err.to_string().contains("OPENAI_API_KEY"),
        "error should mention the rejected env field, got: {err:#}"
    );
}

#[test]
fn manifest_error_names_source_path() {
    let err = parse_toml(
        "tests/fixtures/harnesses/bad-env.toml",
        &fixture("bad-env.toml"),
    )
    .expect_err("bad fixture must fail");

    assert!(
        err.to_string()
            .contains("tests/fixtures/harnesses/bad-env.toml"),
        "error should include source path, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_unknown_field() {
    let err = parse_toml(
        "tests/fixtures/harnesses/unknown-field.toml",
        &fixture("unknown-field.toml"),
    )
    .expect_err("unknown field must fail");

    assert!(
        err.to_string().contains("unknown field"),
        "error should mention unknown field, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_nul_in_args() {
    let input = minimal_manifest("").replace(
        r#"package = "demo-package""#,
        "package = \"demo-package\"\nargs = [\"run\\u0000now\"]",
    );
    let input = input.replace(r#"kind = "npm-global""#, r#"kind = "npx-installer""#);

    let err = parse_toml("nul-args.toml", &input).expect_err("NUL arg must fail");

    assert!(
        err.to_string().contains("package.args"),
        "error should mention package.args, got: {err:#}"
    );
}
