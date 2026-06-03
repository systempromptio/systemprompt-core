//! Terminal rendering for [`CommandOutput`].
//!
//! `json`/`yaml` formats emit the [`CliArtifact`] verbatim; `table` renders per
//! artifact variant for an interactive terminal.

use systemprompt_logging::CliService;
use systemprompt_models::artifacts::{
    ChartArtifact, CliArtifact, ListArtifact, PresentationCardArtifact, TableArtifact,
};

use super::{CommandOutput, value_to_display};
use crate::cli_settings::{OutputFormat, get_global_config};

pub fn render_result(result: &CommandOutput) {
    if result.should_skip_render() {
        return;
    }

    match get_global_config().output_format() {
        OutputFormat::Json => CliService::json(result.artifact()),
        OutputFormat::Yaml => CliService::yaml(result.artifact()),
        OutputFormat::Table => render_terminal(result),
    }
}

fn render_terminal(result: &CommandOutput) {
    if let Some(title) = result.title() {
        CliService::section(title);
    }

    match result.artifact() {
        CliArtifact::Text { artifact } => {
            if result.title().is_none() {
                if let Some(title) = &artifact.title {
                    CliService::section(title);
                }
            }
            CliService::output(&artifact.content);
        },
        CliArtifact::CopyPasteText { artifact } => {
            if result.title().is_none() {
                if let Some(title) = &artifact.title {
                    CliService::section(title);
                }
            }
            CliService::output(&artifact.content);
        },
        CliArtifact::Table { artifact } => render_table(artifact),
        CliArtifact::List { artifact } => render_list(artifact),
        CliArtifact::PresentationCard { artifact } => render_card(artifact),
        CliArtifact::Dashboard { artifact } => {
            if result.title().is_none() {
                CliService::section(&artifact.title);
            }
            if let Some(description) = &artifact.description {
                CliService::output(description);
            }
        },
        CliArtifact::Chart { artifact } => render_chart(artifact),
        CliArtifact::Audio { artifact } => CliService::output(&artifact.src),
        CliArtifact::Image { artifact } => CliService::output(&artifact.src),
        CliArtifact::Video { artifact } => CliService::output(&artifact.src),
        CliArtifact::Message { artifact } => {
            for line in &artifact.messages {
                match line.level.as_str() {
                    "success" => CliService::success(&line.text),
                    "warning" => CliService::warning(&line.text),
                    "error" => CliService::error(&line.text),
                    _ => CliService::info(&line.text),
                }
            }
        },
    }
}

fn render_table(artifact: &TableArtifact) {
    let headers: Vec<&str> = artifact
        .columns
        .iter()
        .map(|c| c.label.as_deref().unwrap_or(&c.name))
        .collect();

    let rows: Vec<Vec<String>> = artifact
        .items
        .iter()
        .map(|item| {
            artifact
                .columns
                .iter()
                .map(|col| {
                    item.get(&col.name)
                        .map_or_else(String::new, value_to_display)
                })
                .collect()
        })
        .collect();

    CliService::table(&headers, &rows);
}

fn render_list(artifact: &ListArtifact) {
    for item in &artifact.items {
        CliService::subsection(&item.title);
        if !item.summary.is_empty() {
            CliService::output(&item.summary);
        }
        if !item.link.is_empty() {
            CliService::output(&item.link);
        }
    }
}

fn render_card(artifact: &PresentationCardArtifact) {
    CliService::section(&artifact.title);
    if let Some(subtitle) = &artifact.subtitle {
        CliService::output(subtitle);
    }
    for section in &artifact.sections {
        CliService::subsection(&section.heading);
        CliService::output(&section.content);
    }
}

fn render_chart(artifact: &ChartArtifact) {
    let mut headers: Vec<&str> = vec!["label"];
    for dataset in &artifact.datasets {
        headers.push(&dataset.label);
    }

    let rows: Vec<Vec<String>> = artifact
        .labels
        .iter()
        .enumerate()
        .map(|(row, label)| {
            let mut cells = vec![label.clone()];
            for dataset in &artifact.datasets {
                cells.push(
                    dataset
                        .data
                        .get(row)
                        .map_or_else(String::new, ToString::to_string),
                );
            }
            cells
        })
        .collect();

    CliService::table(&headers, &rows);
}
