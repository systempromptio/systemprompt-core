//! `admin config catalog discovery` — what a Vertex discovery pass finds
//! with this node's profile and credentials.
//!
//! The report a server builds at boot lives in that server's memory, so the
//! command runs the same read-only listing pass itself (its descriptor asks
//! for model discovery) and reports that outcome: one row per model id with
//! the state it landed in and, for a rate-card id, the retirement date
//! Google's documentation gives it, plus every publisher or credential that
//! failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Context;
use systemprompt_manifest::services::{DiscoveryReport, VertexRateCard};
use systemprompt_models::artifacts::NoticeLine;

use super::types::DiscoveryRow;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

#[must_use]
pub fn discovery_rows(report: &DiscoveryReport, card: &VertexRateCard) -> Vec<DiscoveryRow> {
    let retires_on = |id: &str| -> String {
        card.lookup_id(id)
            .and_then(|entry| entry.retires_on)
            .map(|date| date.to_string())
            .unwrap_or_default()
    };
    let buckets = [
        (&report.discovered_priced, "served"),
        (&report.discovered_unpriced, "unpriced"),
        (&report.priced_not_published, "priced-not-published"),
        (&report.explicit_wins, "explicit"),
        (&report.retiring, "retiring"),
    ];
    buckets
        .into_iter()
        .flat_map(|(ids, state)| {
            ids.iter().map(|id| DiscoveryRow {
                upstream_or_id: id.clone(),
                state: state.to_owned(),
                retires_on: retires_on(id),
            })
        })
        .collect()
}

const fn discovery_rows_empty(report: &DiscoveryReport) -> bool {
    report.discovered_priced.is_empty()
        && report.discovered_unpriced.is_empty()
        && report.priced_not_published.is_empty()
        && report.explicit_wins.is_empty()
        && report.retiring.is_empty()
}

fn discovery_notes(report: &DiscoveryReport) -> Vec<NoticeLine> {
    let mut notes = vec![NoticeLine::new(
        "info",
        format!("ran_at: {}", report.ran_at),
    )];
    if report.failed_publishers.is_empty() {
        notes.push(NoticeLine::new("info", "failed_publishers: none"));
        if discovery_rows_empty(report) {
            notes.push(NoticeLine::new(
                "info",
                "nothing was listed: no enabled provider endpoint on \
                 aiplatform.googleapis.com has a platform credential (a service-account key \
                 secret or workload identity)",
            ));
        }
    } else {
        notes.push(NoticeLine::new(
            "warning",
            format!(
                "failed_publishers: {} (the listing is partial)",
                report.failed_publishers.join(", ")
            ),
        ));
    }
    notes
}

pub fn show_discovery(
    services: Option<&systemprompt_loader::ServicesState>,
    config: &CliConfig,
) -> anyhow::Result<()> {
    let report = services
        .and_then(systemprompt_loader::ServicesState::discovery_report)
        .filter(|r| !r.ran_at.is_empty());
    render_discovery(report, config)
}

pub fn render_discovery(
    report: Option<&DiscoveryReport>,
    config: &CliConfig,
) -> anyhow::Result<()> {
    let card = VertexRateCard::embedded().context("embedded Vertex rate card is unreadable")?;
    let Some(report) = report else {
        render_result(
            &CommandOutput::message(vec![NoticeLine::new(
                "info",
                "Vertex model discovery has not run: the services configuration was not \
                 loaded for this command.",
            )])
            .with_title("Vertex Model Discovery"),
            config,
        );
        return Ok(());
    };
    render_result(
        &CommandOutput::table_of(
            vec!["upstream_or_id", "state", "retires_on"],
            &discovery_rows(report, &card),
        )
        .with_title("Vertex Model Discovery"),
        config,
    );
    render_result(&CommandOutput::message(discovery_notes(report)), config);
    Ok(())
}
