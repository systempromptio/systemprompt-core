//! Brain and notification seams: every registration carries only a priority.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::{brain, notifications};

use crate::kernel::{LinkedSeamRegistration, prioritized};

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_filters",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_filters()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_extractors",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_extractors()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_validators",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_validators()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_importance_models",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_importance_models()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_source_contents",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_source_contents()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_retrievers",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_retrievers()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_rerankers",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_rerankers()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_injection_policies",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_injection_policies()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_outcome_signals",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_outcome_signals()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_analyses",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_analyses()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "brain_consolidators",
    owner: "systemprompt-extension",
    extension: Some("brain"),
    entries: || by_priority!(brain::discover_brain_consolidators()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "notification_routers",
    owner: "systemprompt-extension",
    extension: Some("notifications"),
    entries: || by_priority!(notifications::discover_notification_routers()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "notification_audiences",
    owner: "systemprompt-extension",
    extension: Some("notifications"),
    entries: || by_priority!(notifications::discover_notification_audiences()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "notification_publish_policies",
    owner: "systemprompt-extension",
    extension: Some("notifications"),
    entries: || by_priority!(notifications::discover_notification_publish_policies()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "notification_channels",
    owner: "systemprompt-extension",
    extension: Some("notifications"),
    entries: || by_priority!(notifications::discover_notification_channels()),
});

crate::register_linked_seam!(LinkedSeamRegistration {
    name: "resource_visibilities",
    owner: "systemprompt-extension",
    extension: Some("notifications"),
    entries: || by_priority!(notifications::discover_resource_visibilities()),
});
