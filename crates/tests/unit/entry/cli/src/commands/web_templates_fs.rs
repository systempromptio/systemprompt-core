//! Filesystem-driven tests for the `web templates` command family.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::web::templates::{create, delete, edit, list, selection, show};
use systemprompt_cli::web::types::TemplatesConfig;
use systemprompt_cli::{CliConfig, Prompter};

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn no_answers() -> ScriptedPrompter {
    ScriptedPrompter::new(Vec::<String>::new())
}

fn write_templates_yaml(dir: &Path, yaml: &str) {
    fs::write(dir.join("templates.yaml"), yaml).unwrap();
}

fn read_config(dir: &Path) -> TemplatesConfig {
    serde_yaml::from_str(&fs::read_to_string(dir.join("templates.yaml")).unwrap()).unwrap()
}

const TWO_TEMPLATES: &str = r#"
templates:
  article:
    content_types: [post, page]
  landing:
    content_types: [page]
"#;

#[test]
fn create_adds_template_with_content() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), "templates: {}\n");

    create::execute_in_dir(
        create::CreateArgs {
            name: Some("guide".to_owned()),
            content_types: Some("post, page".to_owned()),
            content: Some("<html>{{TITLE}}</html>".to_owned()),
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap();

    let config = read_config(dir.path());
    assert_eq!(
        config.templates["guide"].content_types,
        vec!["post".to_owned(), "page".to_owned()]
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("guide.html")).unwrap(),
        "<html>{{TITLE}}</html>"
    );
}

#[test]
fn create_reads_content_from_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), "templates: {}\n");
    let src = dir.path().join("source.html");
    fs::write(&src, "<p>from file</p>").unwrap();

    create::execute_in_dir(
        create::CreateArgs {
            name: Some("filed".to_owned()),
            content_types: Some("post".to_owned()),
            content: Some(src.to_string_lossy().to_string()),
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(dir.path().join("filed.html")).unwrap(),
        "<p>from file</p>"
    );
}

#[test]
fn create_rejects_duplicates_and_missing_inputs() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);

    let err = create::execute_in_dir(
        create::CreateArgs {
            name: Some("article".to_owned()),
            content_types: Some("post".to_owned()),
            content: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("already exists"));

    let err = create::execute_in_dir(
        create::CreateArgs {
            name: None,
            content_types: None,
            content: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("--name is required"));

    let err = create::execute_in_dir(
        create::CreateArgs {
            name: Some("fresh".to_owned()),
            content_types: None,
            content: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("--content-types is required"));
}

#[test]
fn create_errors_when_config_missing() {
    let dir = tempfile::tempdir().unwrap();
    let err = create::execute_in_dir(
        create::CreateArgs {
            name: Some("guide".to_owned()),
            content_types: Some("post".to_owned()),
            content: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("Failed to read templates config"));
}

#[test]
fn edit_applies_content_type_changes() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);

    edit::execute_in_dir(
        edit::EditArgs {
            name: Some("article".to_owned()),
            add_content_type: Some("news".to_owned()),
            remove_content_type: Some("page".to_owned()),
            content: None,
            content_types: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap();

    let config = read_config(dir.path());
    assert_eq!(
        config.templates["article"].content_types,
        vec!["post".to_owned(), "news".to_owned()]
    );
}

#[test]
fn edit_replaces_content_types_and_html() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);

    edit::execute_in_dir(
        edit::EditArgs {
            name: Some("landing".to_owned()),
            add_content_type: None,
            remove_content_type: None,
            content: Some("<h1>new</h1>".to_owned()),
            content_types: Some("a, b".to_owned()),
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap();

    let config = read_config(dir.path());
    assert_eq!(
        config.templates["landing"].content_types,
        vec!["a".to_owned(), "b".to_owned()]
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("landing.html")).unwrap(),
        "<h1>new</h1>"
    );
}

#[test]
fn edit_rejects_unknown_template_and_empty_change_set() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);

    let err = edit::execute_in_dir(
        edit::EditArgs {
            name: Some("ghost".to_owned()),
            add_content_type: None,
            remove_content_type: None,
            content: None,
            content_types: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not found"));

    let err = edit::execute_in_dir(
        edit::EditArgs {
            name: Some("article".to_owned()),
            add_content_type: None,
            remove_content_type: None,
            content: None,
            content_types: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("No changes specified"));

    let err = edit::execute_in_dir(
        edit::EditArgs {
            name: Some("article".to_owned()),
            add_content_type: None,
            remove_content_type: Some("missing-type".to_owned()),
            content: None,
            content_types: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not linked to template"));
}

#[test]
fn edit_warns_on_duplicate_add_without_change() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);

    let err = edit::execute_in_dir(
        edit::EditArgs {
            name: Some("article".to_owned()),
            add_content_type: Some("post".to_owned()),
            remove_content_type: None,
            content: None,
            content_types: None,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("No changes specified"));
}

#[test]
fn delete_removes_entry_and_optionally_file() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);
    fs::write(dir.path().join("article.html"), "<p>x</p>").unwrap();

    delete::execute_in_dir(
        delete::DeleteArgs {
            name: Some("article".to_owned()),
            yes: true,
            delete_file: true,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap();

    let config = read_config(dir.path());
    assert!(!config.templates.contains_key("article"));
    assert!(!dir.path().join("article.html").exists());
}

#[test]
fn delete_requires_confirmation_and_existing_template() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);

    let err = delete::execute_in_dir(
        delete::DeleteArgs {
            name: Some("ghost".to_owned()),
            yes: true,
            delete_file: false,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not found"));

    let err = delete::execute_in_dir(
        delete::DeleteArgs {
            name: Some("article".to_owned()),
            yes: false,
            delete_file: false,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("--yes is required"));
}

#[test]
fn show_reports_variables_and_preview() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);
    fs::write(
        dir.path().join("article.html"),
        "<h1>{{TITLE}}</h1>\n<p>{{BODY}}</p>\n<span>{{TITLE}}</span>\n",
    )
    .unwrap();

    let out = show::execute_in_dir(
        show::ShowArgs {
            name: Some("article".to_owned()),
            preview_lines: 2,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert_eq!(json["title"], "Template: article");
}

#[test]
fn show_handles_missing_file_and_unknown_template() {
    let dir = tempfile::tempdir().unwrap();
    write_templates_yaml(dir.path(), TWO_TEMPLATES);

    show::execute_in_dir(
        show::ShowArgs {
            name: Some("landing".to_owned()),
            preview_lines: 5,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap();

    let err = show::execute_in_dir(
        show::ShowArgs {
            name: Some("ghost".to_owned()),
            preview_lines: 5,
        },
        &no_answers(),
        &cfg(),
        dir.path(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn list_reports_missing_files_and_empty_config() {
    let dir = tempfile::tempdir().unwrap();
    list::execute_in_dir(list::ListArgs { missing: false }, &cfg(), dir.path()).unwrap();

    write_templates_yaml(dir.path(), TWO_TEMPLATES);
    fs::write(dir.path().join("article.html"), "<p>x</p>").unwrap();

    let all = list::execute_in_dir(list::ListArgs { missing: false }, &cfg(), dir.path()).unwrap();
    let json = serde_json::to_value(all.artifact()).unwrap();
    assert_eq!(json["items"].as_array().unwrap().len(), 2);

    let missing =
        list::execute_in_dir(list::ListArgs { missing: true }, &cfg(), dir.path()).unwrap();
    let json = serde_json::to_value(missing.artifact()).unwrap();
    let rows = json["items"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "landing");
}

#[test]
fn selection_prompts_over_sorted_names() {
    let config: TemplatesConfig = serde_yaml::from_str(TWO_TEMPLATES).unwrap();
    let prompter = ScriptedPrompter::new(["1"]);
    let picked = selection::prompt_template_selection(&prompter, &config, "pick").unwrap();
    assert_eq!(picked, "landing");

    let empty: TemplatesConfig = serde_yaml::from_str("templates: {}\n").unwrap();
    let err = selection::prompt_template_selection(&no_answers(), &empty, "pick").unwrap_err();
    assert!(err.to_string().contains("No templates configured"));
}

#[test]
fn create_prompt_name_validates_input() {
    let prompter = ScriptedPrompter::new(["BAD NAME", "ok-name"]);
    let name = create::prompt_name(&prompter).unwrap();
    assert_eq!(name, "ok-name");

    let prompter = ScriptedPrompter::new(["a,b , c"]);
    let types = create::prompt_content_types(&prompter as &dyn Prompter).unwrap();
    assert_eq!(types, vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
}
