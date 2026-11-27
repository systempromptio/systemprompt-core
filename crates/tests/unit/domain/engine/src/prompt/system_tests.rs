use std::path::Path;
use std::sync::LazyLock;
use systemprompt_identifiers::ModelId;

use systemprompt_engine::prompt::base::CORE_PROMPT;
use systemprompt_engine::prompt::notices::POLICY_DENIAL_PREFIX;
use systemprompt_engine::prompt::{
    Environment, ExtensionSection, InstructionFile, NamedEntry, ShellInUse, ShellKind, SystemInput,
    build,
};

static MODEL: LazyLock<ModelId> = LazyLock::new(|| ModelId::new("model-large"));

fn environment(shell: Option<ShellInUse<'static>>) -> Environment<'static> {
    Environment {
        working_directory: Path::new(r"C:\work\app\src"),
        worktree: Path::new(r"C:\work\app"),
        is_git: true,
        platform: "windows",
        shell,
        date: "2026-11-25",
        model_id: &MODEL,
    }
}

fn minimal() -> SystemInput<'static> {
    SystemInput {
        agent_prompt: None,
        environment: environment(None),
        instructions: &[],
        extension_sections: &[],
        skills: &[],
        subagents: &[],
        governed_by: None,
        extra: None,
    }
}

const PWSH: ShellInUse<'static> = ShellInUse {
    kind: ShellKind::Pwsh,
    program: "pwsh.exe",
};
const SKILLS: &[NamedEntry<'static>] = &[NamedEntry {
    name: "pdf",
    description: "Read and fill PDF forms.",
}];
const SUBAGENTS: &[NamedEntry<'static>] = &[NamedEntry {
    name: "explore",
    description: "Fast read-only search.",
}];

const SECTIONS: &[ExtensionSection<'static>] = &[ExtensionSection {
    heading: "Memory",
    text: "The user prefers short answers.",
}];

fn full<'a>(instructions: &'a [InstructionFile<'a>]) -> SystemInput<'a> {
    SystemInput {
        agent_prompt: None,
        environment: environment(Some(PWSH)),
        instructions,
        extension_sections: SECTIONS,
        skills: SKILLS,
        subagents: SUBAGENTS,
        governed_by: Some("Example Corp"),
        extra: Some("Answer in British English."),
    }
}

fn heading(block: &str) -> &str {
    block.lines().next().unwrap_or_default()
}

#[test]
fn minimal_input_has_the_core_prompt_and_the_environment() {
    let blocks = build(&minimal());
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0], CORE_PROMPT);
    assert_eq!(heading(&blocks[1]), "# Environment");
    assert!(!blocks[1].contains("Shell:"));
}

#[test]
fn full_input_orders_blocks_from_stable_to_volatile() {
    let instructions = [InstructionFile {
        path: Path::new(r"C:\work\app\AGENTS.md"),
        text: "Use tabs.",
    }];
    let blocks = build(&full(&instructions));
    let headings: Vec<&str> = blocks.iter().map(|block| heading(block)).collect();
    assert_eq!(
        headings,
        [
            heading(CORE_PROMPT),
            "# Shell: PowerShell 7",
            "# Governance",
            "# Instructions",
            "# Memory",
            "# Skills",
            "# Subagents",
            "# Environment",
            "Answer in British English.",
        ]
    );
    assert_eq!(blocks[0], CORE_PROMPT);
}

#[test]
fn build_is_deterministic() {
    let input = full(&[]);
    assert_eq!(build(&input), build(&input));
}

#[test]
fn the_agent_prompt_replaces_the_core_prompt() {
    let input = SystemInput {
        agent_prompt: Some("You review pull requests."),
        ..minimal()
    };
    assert_eq!(build(&input)[0], "You review pull requests.");
    let blank = SystemInput {
        agent_prompt: Some("  \n"),
        ..minimal()
    };
    assert_eq!(build(&blank)[0], CORE_PROMPT);
}

#[test]
fn environment_lists_every_fact() {
    let input = SystemInput {
        environment: environment(Some(PWSH)),
        ..minimal()
    };
    let blocks = build(&input);
    let env = blocks.last().map(String::as_str).unwrap_or_default();
    for fact in [
        r"Working directory: C:\work\app\src",
        r"Workspace root: C:\work\app",
        "Git repository: yes",
        "Platform: windows",
        "Shell: PowerShell 7 (pwsh.exe)",
        "Date: 2026-11-25",
        "Model: model-large",
    ] {
        assert!(env.contains(fact), "missing {fact}");
    }
}

#[test]
fn shell_guidance_follows_the_shell_in_use() {
    let cmd = ShellInUse {
        kind: ShellKind::Cmd,
        program: "cmd.exe",
    };
    let input = SystemInput {
        environment: environment(Some(cmd)),
        ..minimal()
    };
    assert_eq!(build(&input)[1], ShellKind::Cmd.guidance());
}

#[test]
fn instructions_skip_blank_files_and_keep_order() {
    let files = [
        InstructionFile {
            path: Path::new(r"C:\Users\me\AGENTS.md"),
            text: "Be terse.",
        },
        InstructionFile {
            path: Path::new(r"C:\work\app\EMPTY.md"),
            text: "  \n",
        },
        InstructionFile {
            path: Path::new(r"C:\work\app\AGENTS.md"),
            text: "Use tabs.\n",
        },
    ];
    let input = SystemInput {
        instructions: &files,
        ..minimal()
    };
    let blocks = build(&input);
    assert_eq!(blocks.len(), 3);
    let block = &blocks[1];
    assert!(!block.contains("EMPTY.md"));
    let general = block.find("Be terse.").unwrap_or(usize::MAX);
    let specific = block.find("Use tabs.").unwrap_or(0);
    assert!(general < specific);

    let only_blank = [InstructionFile {
        path: Path::new("x.md"),
        text: "",
    }];
    let input = SystemInput {
        instructions: &only_blank,
        ..minimal()
    };
    assert_eq!(build(&input).len(), 2);
}

#[test]
fn skills_and_subagents_list_names_and_descriptions() {
    let input = SystemInput {
        skills: SKILLS,
        subagents: SUBAGENTS,
        ..minimal()
    };
    let blocks = build(&input);
    assert_eq!(blocks.len(), 4);
    assert!(blocks[1].contains("- pdf: Read and fill PDF forms."));
    assert!(blocks[1].contains("skill tool"));
    assert!(blocks[2].contains("- explore: Fast read-only search."));
    assert!(blocks[2].contains("task tool"));
}

#[test]
fn governance_names_the_organisation_and_the_denial_rule() {
    let input = SystemInput {
        governed_by: Some("Example Corp"),
        ..minimal()
    };
    let blocks = build(&input);
    assert_eq!(blocks.len(), 3);
    assert!(blocks[1].contains("Example Corp"));
    assert!(blocks[1].contains(POLICY_DENIAL_PREFIX));
    assert!(blocks[1].contains("do not repeat it"));
    let blank = SystemInput {
        governed_by: Some(" "),
        ..minimal()
    };
    assert_eq!(build(&blank).len(), 2);
}

#[test]
fn extra_text_comes_last_and_blank_extra_is_dropped() {
    let input = SystemInput {
        extra: Some("  Reply in French.  "),
        ..minimal()
    };
    assert_eq!(
        build(&input).last().map(String::as_str),
        Some("Reply in French.")
    );
    let blank = SystemInput {
        extra: Some(""),
        ..minimal()
    };
    assert_eq!(build(&blank).len(), 2);
}

#[test]
fn extension_sections_keep_their_order_and_drop_blank_ones() {
    let sections = [
        ExtensionSection {
            heading: "Drive",
            text: "  Files live in the shared drive.  ",
        },
        ExtensionSection {
            heading: "Empty",
            text: " \n",
        },
        ExtensionSection {
            heading: " ",
            text: "No heading.",
        },
        ExtensionSection {
            heading: "Memory",
            text: "Short answers.",
        },
    ];
    let input = SystemInput {
        extension_sections: &sections,
        ..minimal()
    };
    let blocks = build(&input);
    assert_eq!(blocks.len(), 4);
    assert_eq!(blocks[1], "# Drive\nFiles live in the shared drive.");
    assert_eq!(blocks[2], "# Memory\nShort answers.");
    assert_eq!(heading(&blocks[3]), "# Environment");
}
