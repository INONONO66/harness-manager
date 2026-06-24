//! omx isolation recipe (Codex base + omx state roots).

use crate::harnesses::defs::common;
use crate::isolation::spec::IsolationPlan;

pub(super) fn isolation() -> IsolationPlan {
    IsolationPlan {
        subdir: "omx".to_string(),
        runtime_subdir: "omx".to_string(),
        home_subdirs: vec![".omx".to_string()],
        static_envs: vec![
            common::codex_home_env(),
            ("OMX_AUTO_UPDATE".to_string(), "0".to_string()),
            ("OMX_ROOT".to_string(), "{home}/.omx".to_string()),
            ("OMX_STATE_ROOT".to_string(), "{state}/omx".to_string()),
            (
                "OMX_TEAM_STATE_ROOT".to_string(),
                "{state}/omx-team".to_string(),
            ),
        ],
        seed_files: vec![common::codex_config_seed()],
        caveat: Some(
            "omx harness: Codex CLI with oh-my-codex. Run `omx setup` after first launch if needed."
                .to_string(),
        ),
    }
}
