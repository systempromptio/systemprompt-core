use harness_prompt::engine_tools::{question_schema, skill_content, skill_schema, task_schema};
use serde_json::json;

#[test]
fn task_schema_lists_agents_when_known() {
    let schema = task_schema(&["general", "explore"]);
    assert_eq!(
        schema["properties"]["agent"]["enum"],
        json!(["general", "explore"])
    );
    assert_eq!(
        schema["required"],
        json!(["description", "prompt", "agent"])
    );
    assert!(
        task_schema(&[])["properties"]["agent"]
            .get("enum")
            .is_none()
    );
}

#[test]
fn skill_schema_lists_skills_when_known() {
    assert_eq!(
        skill_schema(&["pdf"])["properties"]["name"]["enum"],
        json!(["pdf"])
    );
    assert!(
        skill_schema(&[])["properties"]["name"]
            .get("enum")
            .is_none()
    );
}

#[test]
fn question_schema_bounds_questions_and_options() {
    let schema = question_schema();
    let questions = &schema["properties"]["questions"];
    assert_eq!(questions["minItems"], json!(1));
    assert_eq!(questions["maxItems"], json!(4));
    let options = &questions["items"]["properties"]["options"];
    assert_eq!(options["minItems"], json!(2));
    assert_eq!(options["maxItems"], json!(4));
}

#[test]
fn skill_content_names_the_base_directory() {
    let text = skill_content("pdf", r"C:\skills\pdf", "\nUse pypdf.\n");
    assert!(text.contains(r"C:\skills\pdf"));
    assert!(text.ends_with("Use pypdf."));
}
