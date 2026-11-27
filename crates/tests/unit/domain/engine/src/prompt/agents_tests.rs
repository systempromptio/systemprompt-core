use systemprompt_engine::prompt::tool_names::{FILE_WRITERS, GREP, QUESTION, READ, SHELL, TASK};
use systemprompt_engine::prompt::{AgentMode, BUILTIN_AGENTS, DEFAULT_AGENT, builtin};

#[test]
fn names_are_unique_and_the_default_exists() {
    for (index, agent) in BUILTIN_AGENTS.iter().enumerate() {
        assert!(
            BUILTIN_AGENTS[index + 1..]
                .iter()
                .all(|other| other.name != agent.name)
        );
    }
    let default = builtin(DEFAULT_AGENT);
    assert!(
        default.is_some_and(|agent| agent.mode.is_primary() && !agent.hidden && !agent.read_only)
    );
    assert!(builtin("missing").is_none());
}

#[test]
fn read_only_agents_cannot_write_files() {
    for agent in BUILTIN_AGENTS.iter().filter(|agent| agent.read_only) {
        for tool in FILE_WRITERS {
            assert!(agent.denies(tool), "{} may call {tool}", agent.name);
        }
    }
}

#[test]
fn utility_agents_are_hidden_and_toolless() {
    for name in ["title", "summary", "compaction"] {
        let agent = builtin(name);
        assert!(
            agent.is_some_and(|agent| agent.hidden && agent.denies(READ) && agent.denies(SHELL))
        );
    }
}

#[test]
fn subagents_cannot_ask_the_user_or_recurse() {
    for agent in BUILTIN_AGENTS
        .iter()
        .filter(|agent| agent.mode == AgentMode::Subagent)
    {
        assert!(agent.denies(QUESTION));
        assert!(agent.denies(TASK));
    }
    assert!(builtin("explore").is_some_and(|agent| !agent.denies(GREP)));
}
