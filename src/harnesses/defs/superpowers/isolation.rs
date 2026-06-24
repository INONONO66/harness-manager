//! superpowers isolation recipe (Codex base).

use crate::harnesses::defs::common;
use crate::isolation::spec::IsolationPlan;

pub(super) fn isolation() -> IsolationPlan {
    IsolationPlan {
        subdir: "superpowers".to_string(),
        runtime_subdir: "superpowers".to_string(),
        home_subdirs: vec![],
        static_envs: vec![common::codex_home_env()],
        seed_files: vec![common::codex_config_seed()],
        caveat: Some(
            "superpowers harness: Codex CLI with the Superpowers plugin installed into the \
             harness-local CODEX_HOME."
                .to_string(),
        ),
    }
}
