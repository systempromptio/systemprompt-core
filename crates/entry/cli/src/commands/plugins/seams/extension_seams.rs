//! Brain and notification seams: every registration carries only a priority.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{brain, notifications};

use super::catalog::{LinkedSeam, prioritized};

pub(super) fn brain_seams() -> Vec<LinkedSeam> {
    let owner = "systemprompt-extension";
    vec![
        LinkedSeam::new(
            "brain_filters",
            owner,
            by_priority!(brain::discover_brain_filters()),
        ),
        LinkedSeam::new(
            "brain_extractors",
            owner,
            by_priority!(brain::discover_brain_extractors()),
        ),
        LinkedSeam::new(
            "brain_validators",
            owner,
            by_priority!(brain::discover_brain_validators()),
        ),
        LinkedSeam::new(
            "brain_importance_models",
            owner,
            by_priority!(brain::discover_brain_importance_models()),
        ),
        LinkedSeam::new(
            "brain_source_contents",
            owner,
            by_priority!(brain::discover_brain_source_contents()),
        ),
        LinkedSeam::new(
            "brain_retrievers",
            owner,
            by_priority!(brain::discover_brain_retrievers()),
        ),
        LinkedSeam::new(
            "brain_rerankers",
            owner,
            by_priority!(brain::discover_brain_rerankers()),
        ),
        LinkedSeam::new(
            "brain_injection_policies",
            owner,
            by_priority!(brain::discover_brain_injection_policies()),
        ),
        LinkedSeam::new(
            "brain_outcome_signals",
            owner,
            by_priority!(brain::discover_brain_outcome_signals()),
        ),
        LinkedSeam::new(
            "brain_analyses",
            owner,
            by_priority!(brain::discover_brain_analyses()),
        ),
        LinkedSeam::new(
            "brain_consolidators",
            owner,
            by_priority!(brain::discover_brain_consolidators()),
        ),
    ]
}

pub(super) fn notification_seams() -> Vec<LinkedSeam> {
    let owner = "systemprompt-extension";
    vec![
        LinkedSeam::new(
            "notification_routers",
            owner,
            by_priority!(notifications::discover_notification_routers()),
        ),
        LinkedSeam::new(
            "notification_audiences",
            owner,
            by_priority!(notifications::discover_notification_audiences()),
        ),
        LinkedSeam::new(
            "notification_publish_policies",
            owner,
            by_priority!(notifications::discover_notification_publish_policies()),
        ),
        LinkedSeam::new(
            "notification_channels",
            owner,
            by_priority!(notifications::discover_notification_channels()),
        ),
        LinkedSeam::new(
            "resource_visibilities",
            owner,
            by_priority!(notifications::discover_resource_visibilities()),
        ),
    ]
}
