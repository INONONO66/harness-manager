//! gstack-claude isolation recipe (Claude base).

use crate::harnesses::defs::common;
use crate::isolation::spec::IsolationPlan;

pub(super) fn isolation() -> IsolationPlan {
    IsolationPlan {
        subdir: "gstack-claude".to_string(),
        runtime_subdir: "gstack-claude".to_string(),
        home_subdirs: vec![],
        static_envs: common::claude_static_envs(),
        seed_files: vec![common::claude_settings_seed(
            "{\n  \"permissions\": { \"defaultMode\": \"ask\" }\n}\n",
        )],
        caveat: Some(
            "gstack-claude harness: Claude Code with gstack skills installed into the \
             harness-local HOME."
                .to_string(),
        ),
    }
}
