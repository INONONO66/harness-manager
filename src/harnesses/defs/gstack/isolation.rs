//! gstack isolation recipe (Codex base).

use crate::harnesses::defs::common;
use crate::isolation::spec::IsolationPlan;

pub(super) fn isolation() -> IsolationPlan {
    IsolationPlan {
        subdir: "gstack".to_string(),
        runtime_subdir: "gstack".to_string(),
        home_subdirs: vec![],
        static_envs: vec![common::codex_home_env()],
        seed_files: vec![common::codex_config_seed()],
        caveat: Some(
            "gstack harness: Codex CLI with gstack skills installed into the harness-local \
             CODEX_HOME."
                .to_string(),
        ),
    }
}
