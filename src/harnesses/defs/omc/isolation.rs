//! omc isolation recipe (Claude base + MCP-server guard + omc settings seed).

use crate::harnesses::defs::common;
use crate::isolation::spec::IsolationPlan;

pub(super) fn isolation() -> IsolationPlan {
    let mut static_envs = common::claude_static_envs();
    static_envs.push((
        "ENABLE_CLAUDEAI_MCP_SERVERS".to_string(),
        "false".to_string(),
    ));
    IsolationPlan {
        subdir: "omc".to_string(),
        runtime_subdir: "omc".to_string(),
        home_subdirs: vec![],
        static_envs,
        seed_files: vec![common::claude_settings_seed(
            "{\n  \"permissions\": { \"defaultMode\": \"ask\" },\n  \"enabledPlugins\": { \"oh-my-claudecode\": true }\n}\n",
        )],
        caveat: Some(
            "omc harness: Claude Code with oh-my-claudecode plugin. Install omc first: \
             hm harness install omc"
                .to_string(),
        ),
    }
}
