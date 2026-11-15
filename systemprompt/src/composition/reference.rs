//! The reference composition: every core member of this build, in a
//! topological order of the members' declared dependency edges, with the terms
//! the conferral table grants it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::reserved::CoreId;

use super::{Composition, CoreTerms, Member};

#[must_use]
pub fn reference() -> Composition {
    let mut members: Vec<Member> = Vec::new();
    #[cfg(feature = "database")]
    members.push(Member::core::<systemprompt_database::DatabaseExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "events")]
    members.push(Member::core::<systemprompt_events::EventsExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_users::UsersExtension>(
        CoreTerms::REQUIRED,
    ));
    #[cfg(feature = "logging")]
    members.push(Member::core::<systemprompt_logging::LoggingExtension>(
        CoreTerms::REQUIRED,
    ));
    #[cfg(feature = "security")]
    members
        .push(Member::core::<systemprompt_governance::authz::AuthzExtension>(CoreTerms::REQUIRED));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_oauth::OauthExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "api")]
    members
        .push(Member::core::<systemprompt_oauth_issuance::IdentityExtension>(CoreTerms::OPTIONAL));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_mcp::McpExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_ai::AiExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_agent::AgentExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "analytics")]
    members.push(Member::core::<systemprompt_analytics::AnalyticsExtension>(
        CoreTerms::REQUIRED,
    ));
    #[cfg(feature = "content")]
    members.push(Member::core::<systemprompt_content::ContentExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_files::FilesExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "brain")]
    members.push(Member::core::<systemprompt_brain::BrainExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "notifications")]
    members.push(Member::core::<
        systemprompt_notifications::NotificationsExtension,
    >(CoreTerms::OPTIONAL));
    #[cfg(feature = "full")]
    members.push(Member::core::<
        systemprompt_marketplace::ManagedResourcesExtension,
    >(CoreTerms::OPTIONAL));
    #[cfg(feature = "api")]
    members.push(Member::core::<
        systemprompt_gateway::extension::GatewayExtension,
    >(CoreTerms::OPTIONAL));
    #[cfg(feature = "agent-runs")]
    members.push(Member::core::<systemprompt_agent_runs::AgentRunsExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_generator::GeneratorExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "full")]
    members.push(Member::core::<systemprompt_scheduler::SchedulerExtension>(
        CoreTerms::OPTIONAL,
    ));
    #[cfg(feature = "content")]
    members.push(Member::core::<
        systemprompt_content_entry::extension::ContentHttpExtension,
    >(CoreTerms::conferred(CoreId::ContentHttp)));
    #[cfg(feature = "analytics")]
    members.push(Member::core::<
        systemprompt_analytics_entry::extension::AnalyticsHttpExtension,
    >(CoreTerms::conferred(CoreId::AnalyticsHttp)));
    #[cfg(feature = "notifications")]
    members.push(Member::core::<
        systemprompt_notifications_entry::extension::NotificationsHttpExtension,
    >(CoreTerms::conferred(CoreId::NotificationsHttp)));
    #[cfg(all(feature = "agent-runs", feature = "api"))]
    members.push(Member::core::<
        systemprompt_agent_runs_entry::http::AgentRunsApiExtension,
    >(CoreTerms::conferred(CoreId::AgentRunsApi)));
    #[cfg(all(feature = "cloud", feature = "cli"))]
    members.push(Member::core::<systemprompt_cloud_entry::CloudCliExtension>(
        CoreTerms::conferred(CoreId::Cloud),
    ));
    #[cfg(feature = "slack")]
    members.push(Member::core::<systemprompt_slack_entry::SlackExtension>(
        CoreTerms::conferred(CoreId::Slack),
    ));
    #[cfg(feature = "teams")]
    members.push(Member::core::<systemprompt_teams_entry::TeamsExtension>(
        CoreTerms::conferred(CoreId::Teams),
    ));
    Composition {
        members,
        removed: Vec::new(),
        #[cfg(feature = "config")]
        secrets: Vec::new(),
    }
}
