//! omo isolation recipe (OpenCode config dir + autoupdate/project guards).

use crate::isolation::spec::IsolationPlan;

pub(super) fn isolation() -> IsolationPlan {
    IsolationPlan {
        subdir: "omo".to_string(),
        runtime_subdir: "omo".to_string(),
        home_subdirs: vec![],
        static_envs: vec![
            (
                "OPENCODE_CONFIG_DIR".to_string(),
                "{home}/.config/opencode".to_string(),
            ),
            ("OPENCODE_DISABLE_AUTOUPDATE".to_string(), "1".to_string()),
            (
                "OPENCODE_DISABLE_PROJECT_CONFIG".to_string(),
                "1".to_string(),
            ),
        ],
        seed_files: vec![],
        caveat: Some("omo harness: OpenCode with oh-my-openagent plugin.".to_string()),
    }
}
