use crate::isolation::subst_tokens;

use super::tmp_paths;

#[test]
fn subst_tokens_replaces_home() {
    let p = tmp_paths("subst-home");
    let result = subst_tokens("{home}/.codex/config.toml", &p);

    assert!(
        result.ends_with("/home/.codex/config.toml"),
        "got: {}",
        result
    );
    assert!(!result.contains("{home}"));
}

#[test]
fn subst_tokens_replaces_state_and_tmp() {
    let p = tmp_paths("subst-state-tmp");

    assert!(subst_tokens("{state}/logs", &p).ends_with("/state/logs"));
    assert!(subst_tokens("{tmp}/foo", &p).ends_with("/tmp/foo"));
}

#[test]
fn subst_tokens_replaces_runtime_state_and_logs() {
    let p = tmp_paths("subst-runtime-logs");

    assert!(subst_tokens("{runtime_home}/.codex", &p).ends_with("/home/.codex"));
    assert!(subst_tokens("{runtime_state}/sessions", &p).ends_with("/state/sessions"));
    assert!(subst_tokens("{runtime_logs}", &p).ends_with("/state/logs"));
}

#[test]
fn subst_tokens_passes_through_unknown_and_plain() {
    let p = tmp_paths("subst-unknown");

    assert_eq!(subst_tokens("OPENCODE_PURE", &p), "OPENCODE_PURE");
    assert_eq!(subst_tokens("1", &p), "1");
    assert_eq!(subst_tokens("{unknown}/x", &p), "{unknown}/x");
}
