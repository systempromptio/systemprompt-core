//! Interactive template deletion preserves bytes on cancellation and optionally
//! retains HTML.

use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::web::templates::delete::{self, DeleteArgs};
use systemprompt_cli::{CliConfig, OutputFormat};

#[test]
fn cancelled_then_confirmed_metadata_deletion_preserves_the_html_file() {
    let dir = tempfile::tempdir().expect("owned templates directory");
    let config_path = dir.path().join("templates.yaml");
    let html_path = dir.path().join("article.html");
    std::fs::write(&config_path, "templates:\n  article:\n    content_types: [post]\n  sibling:\n    content_types: [page]\n").expect("templates config");
    std::fs::write(&html_path, b"<article>owned bytes</article>").expect("article HTML");
    let config_before = std::fs::read(&config_path).expect("config before");
    let html_before = std::fs::read(&html_path).expect("HTML before");
    let interactive = CliConfig::new()
        .with_interactive(true)
        .with_assume_terminal(true)
        .with_output_format(OutputFormat::Json);

    let cancelled = delete::execute_in_dir(
        DeleteArgs {
            name: Some("article".to_owned()),
            yes: false,
            delete_file: false,
        },
        &ScriptedPrompter::new(["no"]),
        &interactive,
        dir.path(),
    )
    .expect_err("declined deletion must cancel");
    assert!(format!("{cancelled:#}").contains("Operation cancelled"));
    assert_eq!(
        std::fs::read(&config_path).expect("config after cancel"),
        config_before
    );
    assert_eq!(
        std::fs::read(&html_path).expect("HTML after cancel"),
        html_before
    );

    let output = delete::execute_in_dir(
        DeleteArgs {
            name: Some("article".to_owned()),
            yes: false,
            delete_file: false,
        },
        &ScriptedPrompter::new(["yes"]),
        &interactive,
        dir.path(),
    )
    .expect("confirmed metadata deletion");
    let artifact = serde_json::to_value(output.artifact()).expect("delete artifact");
    let sections = artifact["sections"].as_array().expect("delete sections");
    let field = |heading: &str| {
        sections
            .iter()
            .find(|s| s["heading"] == heading)
            .unwrap_or_else(|| panic!("missing {heading}: {artifact}"))["content"]
            .clone()
    };
    assert_eq!(field("deleted"), "article");
    assert_eq!(field("file_deleted"), false);
    assert_eq!(
        field("message"),
        format!(
            "Template 'article' deleted. HTML file still exists at {}",
            html_path.display()
        )
    );
    let parsed: serde_yaml::Value =
        serde_yaml::from_slice(&std::fs::read(&config_path).expect("config after delete"))
            .expect("parse config");
    assert!(parsed["templates"]["article"].is_null());
    assert_eq!(parsed["templates"]["sibling"]["content_types"][0], "page");
    assert_eq!(
        std::fs::read(&html_path).expect("retained HTML"),
        html_before
    );
}
