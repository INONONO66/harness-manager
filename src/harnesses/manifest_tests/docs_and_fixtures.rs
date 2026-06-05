use std::path::Path;

use super::parse_toml;

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
    // Given: the manifest documentation's first TOML example.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("harness-manifest.md");
    let docs = std::fs::read_to_string(path).expect("manifest docs exist");
    let demo = first_toml_block(&docs);

    // When: the documented example is parsed.
    let parsed = parse_toml("docs/harness-manifest.md", demo).expect("documented demo parses");

    // Then: it is still the demo harness.
    assert_eq!(parsed.id, "demo");
}

#[test]
fn manifest_rejects_path_launch_binary() {
    // Given: a fixture with a path-shaped launch binary.
    let err = parse_toml(
        "tests/fixtures/harnesses/bad-launch.toml",
        &fixture("bad-launch.toml"),
    )
    .expect_err("path launch binary must fail");

    // Then: the launch_binary field is named in the error.
    assert!(
        err.to_string().contains("launch_binary"),
        "error should mention launch_binary, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_secret_static_env() {
    // Given: a fixture that tries to hardcode a secret env var.
    let err = parse_toml(
        "tests/fixtures/harnesses/bad-env.toml",
        &fixture("bad-env.toml"),
    )
    .expect_err("secret static env must fail");

    // Then: the rejected env field is named.
    assert!(
        err.to_string().contains("OPENAI_API_KEY"),
        "error should mention the rejected env field, got: {err:#}"
    );
}

#[test]
fn manifest_error_names_source_path() {
    // Given: a bad fixture parsed with its source label.
    let err = parse_toml(
        "tests/fixtures/harnesses/bad-env.toml",
        &fixture("bad-env.toml"),
    )
    .expect_err("bad fixture must fail");

    // Then: the source path remains in the error.
    assert!(
        err.to_string()
            .contains("tests/fixtures/harnesses/bad-env.toml"),
        "error should include source path, got: {err:#}"
    );
}

#[test]
fn manifest_rejects_unknown_field() {
    // Given: a fixture containing an unknown manifest field.
    let err = parse_toml(
        "tests/fixtures/harnesses/unknown-field.toml",
        &fixture("unknown-field.toml"),
    )
    .expect_err("unknown field must fail");

    // Then: serde's unknown-field error is preserved.
    assert!(
        err.to_string().contains("unknown field"),
        "error should mention unknown field, got: {err:#}"
    );
}

