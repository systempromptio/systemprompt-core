#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::web::content_types::{ContentTypesCommands, execute};
use systemprompt_cli::{CliConfig, OutputFormat, ScriptedPrompter};
use systemprompt_models::content_config::ContentConfigRaw;

const RICH: &str = r#"content_sources:
  guides:
    path: content/guides
    source_id: guides
    category_id: guides
    enabled: true
    description: Long-form guides
    allowed_content_types: [guide]
    indexing:
      clear_before: true
      recursive: true
      override_existing: false
    branding:
      name: Guides
      description: Everything we know
      image: /img/guides.png
      keywords: guides,howto
    sitemap:
      enabled: true
      url_pattern: /guides/{slug}
      priority: 0.8
      changefreq: weekly
      fetch_from: database
      parent_route:
        enabled: true
        url: /guides
        priority: 0.9
        changefreq: daily
categories:
  guides:
    name: guides
"#;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: ContentTypesCommands,
}

fn seed() -> ContentConfigRaw {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    std::fs::write(boot.services_path.join("content/config.yaml"), RICH).unwrap();
    serde_yaml::from_str(RICH).unwrap()
}

fn run(args: &[&str], config: &CliConfig, answers: Vec<String>) -> anyhow::Result<()> {
    let cmd = Harness::try_parse_from(std::iter::once("content-types").chain(args.iter().copied()))
        .unwrap()
        .cmd;
    let prompter = ScriptedPrompter::new(answers);
    execute(
        cmd,
        &prompter,
        config,
        systemprompt_test_fixtures::ensure_test_bootstrap()
            .boot
            .profile(),
    )
}

#[test]
fn show_renders_sitemap_parent_route_branding_and_indexing() {
    let seeded = seed();
    let guides = &seeded.content_sources["guides"];
    assert!(guides.sitemap.as_ref().unwrap().parent_route.is_some());
    assert!(guides.branding.is_some());
    assert!(guides.indexing.is_some());

    for format in [OutputFormat::Json, OutputFormat::Table] {
        let config = CliConfig::new()
            .with_interactive(false)
            .with_output_format(format);
        run(&["show", "guides"], &config, Vec::new()).expect("show renders");
    }
}

#[test]
fn show_without_a_name_prompts_for_the_type_when_interactive() {
    seed();
    let config = CliConfig::new()
        .with_interactive(true)
        .with_assume_terminal(true)
        .with_output_format(OutputFormat::Json);
    run(&["show"], &config, vec!["0".to_owned()]).expect("prompted selection");
}

#[test]
fn show_without_a_name_is_refused_when_not_interactive() {
    seed();
    let config = CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json);
    let err = run(&["show"], &config, Vec::new()).expect_err("name required");
    assert!(format!("{err:#}").contains("name"), "{err:#}");
}
