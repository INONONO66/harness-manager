use super::dynamic::RuntimeRegistry;

#[test]
fn builtin_only_returns_expected_runtimes() {
    let registry = RuntimeRegistry::builtin_only().expect("builtin runtimes parse");
    let names: Vec<&str> = registry.records().iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"Claude Code"));
    assert!(names.contains(&"Codex CLI"));
    assert!(names.contains(&"Gajae-Code"));
    assert!(names.contains(&"Grok CLI"));
    assert!(names.contains(&"OpenCode"));
    assert!(names.contains(&"Pi"));
    assert_eq!(registry.records().len(), 6);
}

#[test]
fn find_by_binary_and_display_name() {
    let registry = RuntimeRegistry::builtin_only().unwrap();
    assert_eq!(registry.find("claude").unwrap().name, "Claude Code");
    assert_eq!(registry.find("Claude Code").unwrap().name, "Claude Code");
    assert_eq!(registry.find("codex").unwrap().name, "Codex CLI");
    assert_eq!(registry.find("Codex CLI").unwrap().name, "Codex CLI");
    assert_eq!(registry.find("CODEX-CLI").unwrap().name, "Codex CLI");
    assert_eq!(registry.find("gjc").unwrap().name, "Gajae-Code");
    assert_eq!(registry.find("grok").unwrap().name, "Grok CLI");
    assert!(registry.find("nope").is_none());
}

#[test]
fn find_by_display_name_is_exact_match() {
    let registry = RuntimeRegistry::builtin_only().unwrap();
    assert!(registry.find_by_display_name("Claude Code").is_some());
    assert!(registry.find_by_display_name("claude").is_none());
}

#[test]
fn id_conflicts_with_runtime_blocks_runtime_binary_and_display_name() {
    let registry = RuntimeRegistry::builtin_only().unwrap();
    assert!(registry.id_conflicts_with_runtime("codex"));
    assert!(registry.id_conflicts_with_runtime("claudecode"));
    assert!(!registry.id_conflicts_with_runtime("my-harness"));
}
