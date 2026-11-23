//! Every `CoreId` names a distinct extension id, so the kernel's owner
//! re-check (owner id equals `CoreId::id()`) can never confuse two core
//! members.

use std::collections::HashSet;

use systemprompt_extension::reserved::CoreId;

const ALL: [CoreId; 15] = [
    CoreId::ContentHttp,
    CoreId::AnalyticsHttp,
    CoreId::NotificationsHttp,
    CoreId::AgentRunsApi,
    CoreId::Slack,
    CoreId::Teams,
    CoreId::Cloud,
    CoreId::MarketplaceHttp,
    CoreId::FilesHttp,
    CoreId::McpHttp,
    CoreId::AgentHttp,
    CoreId::BrainHttp,
    CoreId::GatewayHttp,
    CoreId::IdentityHttp,
    CoreId::Kernel,
];

const fn listed(id: CoreId) -> usize {
    match id {
        CoreId::ContentHttp => 0,
        CoreId::AnalyticsHttp => 1,
        CoreId::NotificationsHttp => 2,
        CoreId::AgentRunsApi => 3,
        CoreId::Slack => 4,
        CoreId::Teams => 5,
        CoreId::Cloud => 6,
        CoreId::MarketplaceHttp => 7,
        CoreId::FilesHttp => 8,
        CoreId::McpHttp => 9,
        CoreId::AgentHttp => 10,
        CoreId::BrainHttp => 11,
        CoreId::GatewayHttp => 12,
        CoreId::IdentityHttp => 13,
        CoreId::Kernel => 14,
    }
}

#[test]
fn the_list_holds_every_variant_once() {
    for (index, id) in ALL.iter().enumerate() {
        assert_eq!(listed(*id), index);
    }
}

#[test]
fn core_id_values_are_pairwise_distinct() {
    let ids: HashSet<&str> = ALL.iter().map(|id| id.id()).collect();
    assert_eq!(
        ids.len(),
        ALL.len(),
        "two CoreId variants share an id: {ids:?}"
    );
}

#[test]
fn core_id_values_are_valid_extension_ids() {
    for id in ALL {
        assert!(
            systemprompt_identifiers::ExtensionId::try_new(id.id()).is_ok(),
            "{id:?} names an invalid extension id"
        );
    }
}
