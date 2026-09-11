//! `admin config catalog discovery` — what the last Vertex discovery pass
//! found.
//!
//! One row per model id with the state it landed in and, for a rate-card id,
//! the retirement date Google's documentation gives it.
//!
//! The scheduler's daily report is preferred over the boot-time one: both
//! describe the same upstream, but the scheduler's is fresher and the boot
//! report never changes for the life of the process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::artifacts::NoticeLine;
use systemprompt_models::services::{DiscoveryReport, VertexRateCard};

use super::types::DiscoveryRow;
use crate::CliConfig;
use crate::shared::{CommandOutput, render_result};

fn latest_discovery() -> Option<DiscoveryReport> {
    systemprompt_scheduler::jobs::vertex_discovery::latest_report().or_else(|| {
        systemprompt_loader::ServicesBootstrap::discovery_report()
            .filter(|r| !r.ran_at.is_empty())
            .cloned()
    })
}

#[must_use]
pub fn discovery_rows(report: &DiscoveryReport) -> Vec<DiscoveryRow> {
    let card = VertexRateCard::embedded().ok();
    let retires_on = |id: &str| -> String {
        card.as_ref()
            .and_then(|card| card.lookup_id(id))
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

fn discovery_notes(report: &DiscoveryReport) -> Vec<NoticeLine> {
    let mut notes = vec![NoticeLine::new(
        "info",
        format!("ran_at: {}", report.ran_at),
    )];
    if report.failed_publishers.is_empty() {
        notes.push(NoticeLine::new("info", "failed_publishers: none"));
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

pub fn show_discovery(config: &CliConfig) {
    let Some(report) = latest_discovery() else {
        render_result(
            &CommandOutput::message(vec![NoticeLine::new(
                "info",
                "Vertex model discovery has not run. It runs when the server boots, and only \
                 when a provider endpoint is on aiplatform.googleapis.com and its secret is \
                 a Google service-account key.",
            )])
            .with_title("Vertex Model Discovery"),
            config,
        );
        return;
    };
    render_result(
        &CommandOutput::table_of(
            vec!["upstream_or_id", "state", "retires_on"],
            &discovery_rows(&report),
        )
        .with_title("Vertex Model Discovery"),
        config,
    );
    render_result(&CommandOutput::message(discovery_notes(&report)), config);
}
