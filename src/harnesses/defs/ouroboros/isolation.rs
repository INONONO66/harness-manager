//! ouroboros isolation recipe (Codex base + ouroboros home dir).

use crate::harnesses::defs::common;
use crate::isolation::spec::IsolationPlan;

pub(super) fn isolation() -> IsolationPlan {
    IsolationPlan {
        subdir: "ouroboros".to_string(),
        runtime_subdir: "ouroboros".to_string(),
        home_subdirs: vec![".ouroboros".to_string()],
        static_envs: vec![common::codex_home_env()],
        seed_files: vec![common::codex_config_seed()],
        caveat: Some(
            "ouroboros harness: Codex CLI with ouroboros workflow engine. Run \
             `ouroboros setup --runtime codex` after install."
                .to_string(),
        ),
    }
}
