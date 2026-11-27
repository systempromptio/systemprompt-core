//! Descriptions and input schemas of the tools the engine implements itself:
//! `task`, `skill` and `question`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};

pub const TASK_DESCRIPTION: &str = r"Start a subagent that carries out a task on its own and returns a report. The available subagents are listed in the system prompt under Subagents; choose one by name in agent.
Good uses: a broad search of unfamiliar code, research that would fill your context with material you will not need afterwards, and independent parts of a larger job. Independent tasks run in parallel when you start them in the same turn.
Do it yourself instead when it takes one or two calls, such as reading a known file or a single search.
The subagent knows nothing of this conversation. Write prompt as a complete brief: the goal, what is already known, the relevant paths, constraints, whether it may change files, and what its report should contain. Only its final message comes back to you, and the user does not see it, so pass on what matters.";

// JSON: the task tool input schema sent to the model provider verbatim.
pub fn task_schema(agent_names: &[&str]) -> Value {
    let mut agent = json!({
        "type": "string",
        "description": "Name of the subagent to run, from the Subagents list."
    });
    if !agent_names.is_empty() {
        agent["enum"] = json!(agent_names);
    }
    json!({
        "type": "object",
        "properties": {
            "description": {
                "type": "string",
                "description": "A short label for the task, three to six words, shown to the user."
            },
            "prompt": {
                "type": "string",
                "description": "The full brief for the subagent."
            },
            "agent": agent
        },
        "required": ["description", "prompt", "agent"],
        "additionalProperties": false
    })
}

pub const SKILL_DESCRIPTION: &str = r"Load a skill: packaged instructions, sometimes with scripts and reference files, for a particular kind of task. The available skills are listed in the system prompt under Skills, each with a description of when it applies.
When a task matches a skill's description, load the skill before you start and follow its instructions. A loaded skill stays in the conversation, so load each one only once. Files a skill mentions are relative to the base directory given with its text.";

// JSON: the skill tool input schema sent to the model provider verbatim.
pub fn skill_schema(skill_names: &[&str]) -> Value {
    let mut name = json!({
        "type": "string",
        "description": "Name of the skill to load, exactly as listed."
    });
    if !skill_names.is_empty() {
        name["enum"] = json!(skill_names);
    }
    json!({
        "type": "object",
        "properties": { "name": name },
        "required": ["name"],
        "additionalProperties": false
    })
}

pub fn skill_content(name: &str, base_directory: &str, body: &str) -> String {
    format!(
        "Skill {name} is loaded. Its base directory is {base_directory}; paths in the skill are relative to it.\n\n{}",
        body.trim()
    )
}

pub const QUESTION_MAX_QUESTIONS: usize = 4;
pub const QUESTION_MIN_OPTIONS: usize = 2;
pub const QUESTION_MAX_OPTIONS: usize = 4;

pub const QUESTION_DESCRIPTION: &str = r#"Ask the user one to four questions and wait for the answers. Each question offers two to four options; the user can always type an answer of their own instead, so never add an "Other" option.
Use it when you need a decision or preference only the user can give, when the request is ambiguous in a way that changes the result, or to confirm a risky step. Do not use it for things you can find out yourself or to ask leave for routine steps.
Put the option you recommend first and end its label with "(recommended)". Group related questions into one call rather than asking them one at a time."#;

// JSON: the question tool input schema sent to the model provider verbatim.
pub fn question_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "questions": {
                "type": "array",
                "minItems": 1,
                "maxItems": QUESTION_MAX_QUESTIONS,
                "items": {
                    "type": "object",
                    "properties": {
                        "question": {
                            "type": "string",
                            "description": "The full question, ending with a question mark."
                        },
                        "header": {
                            "type": "string",
                            "description": "A very short label for the question, at most 12 characters, such as Database or Scope."
                        },
                        "options": {
                            "type": "array",
                            "minItems": QUESTION_MIN_OPTIONS,
                            "maxItems": QUESTION_MAX_OPTIONS,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "label": {
                                        "type": "string",
                                        "description": "The choice, in one to five words."
                                    },
                                    "description": {
                                        "type": "string",
                                        "description": "What choosing it means, or its trade-off."
                                    }
                                },
                                "required": ["label", "description"],
                                "additionalProperties": false
                            }
                        },
                        "multiple": {
                            "type": "boolean",
                            "description": "True when the user may pick more than one option."
                        }
                    },
                    "required": ["question", "header", "options"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["questions"],
        "additionalProperties": false
    })
}
