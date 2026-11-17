//! The schema composition every test database is installed from.
//!
//! The test workspace cannot take the `systemprompt` facade (its reference
//! composition needs the facade's `full` feature), so [`core_members`] lists
//! the schema-bearing core members of `systemprompt::reference()` in the same
//! order. [`schema_admitted`] admits them with nothing disabled and no
//! profile, and [`install_core_schemas`] installs them through the one install
//! path, `install_admitted_schemas`, which also writes the table-ownership and
//! purge records.

use systemprompt_database::{
    DatabaseProvider, MigrationConfig, SchemaInstallReport, install_admitted_schemas,
};
use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts};
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{
    Admitted, ContributionError, Contributions, Extension, LoaderError, StandingTag,
};
use systemprompt_identifiers::ExtensionId;

pub fn core_members() -> Vec<Box<dyn Extension>> {
    vec![
        Box::new(systemprompt_database::DatabaseExtension::default()),
        Box::new(systemprompt_events::EventsExtension::default()),
        Box::new(systemprompt_users::UsersExtension::default()),
        Box::new(systemprompt_logging::LoggingExtension::default()),
        Box::new(systemprompt_governance::authz::AuthzExtension::default()),
        Box::new(systemprompt_oauth::OauthExtension::default()),
        Box::new(systemprompt_mcp::McpExtension::default()),
        Box::new(systemprompt_ai::AiExtension::default()),
        Box::new(systemprompt_agent::AgentExtension::default()),
        Box::new(systemprompt_analytics::AnalyticsExtension::default()),
        Box::new(systemprompt_content::ContentExtension::default()),
        Box::new(systemprompt_files::FilesExtension::default()),
        Box::new(systemprompt_brain::BrainExtension::default()),
        Box::new(systemprompt_notifications::NotificationsExtension::default()),
        Box::new(systemprompt_marketplace::ManagedResourcesExtension::default()),
        Box::new(systemprompt_agent_runs::AgentRunsExtension::default()),
        Box::new(systemprompt_scheduler::SchedulerExtension::default()),
    ]
}

pub fn admitted_member(
    ext: &dyn Extension,
    standing: StandingTag,
) -> Result<AdmittedMember, ContributionError> {
    let metadata = ext.metadata();
    let owner = ExtensionId::new(metadata.id);
    let mut sink = ContributionSink::default();
    ext.contribute(&mut Contributions::__new(owner.clone(), &mut sink))?;
    if let Some(error) = sink.take_errors().into_iter().next() {
        return Err(error);
    }
    Ok(AdmittedMember {
        owner,
        metadata,
        standing,
        sink,
    })
}

pub fn schema_admitted() -> Result<Admitted, ContributionError> {
    let members = core_members()
        .iter()
        .map(|ext| admitted_member(ext.as_ref(), StandingTag::Core))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Admitted::__from_facade(AdmittedParts {
        members,
        ..AdmittedParts::default()
    }))
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("a core member's contributions could not be collected")]
    Contribution(#[from] ContributionError),
    #[error("schema installation failed")]
    Install(#[from] LoaderError),
}

pub async fn install_core_schemas(
    db: &dyn DatabaseProvider,
) -> Result<SchemaInstallReport, InstallError> {
    let admitted = schema_admitted()?;
    Ok(install_admitted_schemas(admitted.built(), db, MigrationConfig::default()).await?)
}
