//! The non-secret admitted views a member's command reads in place of the
//! composition: only admitted members' migrations, configuration sections
//! and assets, served open through a member command's resolver.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use systemprompt_cli::kernel::MemberHost;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{CliConfig, CommandDescriptor, CommandHost, OutputFormat};
use systemprompt_extension::{
    Admitted, AdmittedAssets, AdmittedConfigs, AdmittedMigrations, AssetDefinition, AssetPaths,
    Migration,
};

use crate::probe::{Probe, profile};

const PROBE: &str = "probe-views";

struct Paths(PathBuf);

impl AssetPaths for Paths {
    fn storage_files(&self) -> &Path {
        &self.0
    }

    fn web_dist(&self) -> &Path {
        &self.0
    }
}

fn admitted(disabled: &[&str]) -> Admitted {
    systemprompt::reference()
        .with(Probe::new(PROBE, |c| {
            c.migrations(vec![Migration::new(1, "probe_views", "SELECT 1")]);
            c.asset_with(|paths| {
                vec![AssetDefinition::css(
                    paths.storage_files().join("probe.css"),
                    "probe.css",
                )]
            });
        }))
        .build(&profile(disabled))
        .expect("the composition admits")
}

fn member_ctx(admitted: Admitted) -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_admitted(Some(Arc::new(admitted)))
    .with_detached_kernel()
}

#[test]
fn an_admitted_members_migrations_and_assets_are_in_its_views() {
    let admitted = admitted(&[]);
    let migrations = AdmittedMigrations::of(admitted.built());
    assert!(
        migrations
            .members()
            .iter()
            .any(|(meta, list)| meta.id == PROBE && list.iter().any(|m| m.version == 1))
    );
    let assets = AdmittedAssets::of(admitted.built());
    let required = assets.required(&Paths(PathBuf::from("/srv")));
    assert!(
        required.iter().any(|(owner, asset)| owner.as_str() == PROBE
            && asset.source() == Path::new("/srv/probe.css"))
    );
    let configs = AdmittedConfigs::of(admitted.built());
    let declared: usize = admitted
        .built()
        .members()
        .map(|member| member.sink.configs().len())
        .sum();
    assert_eq!(configs.decls().len(), declared);
}

#[test]
fn a_disabled_members_declarations_are_absent_from_its_views() {
    let admitted = admitted(&[PROBE]);
    let migrations = AdmittedMigrations::of(admitted.built());
    assert!(
        !migrations
            .members()
            .iter()
            .any(|(meta, _)| meta.id == PROBE)
    );
    let required = AdmittedAssets::of(admitted.built()).required(&Paths(PathBuf::from("/srv")));
    assert!(!required.iter().any(|(owner, _)| owner.as_str() == PROBE));
}

#[test]
fn a_member_command_reads_the_open_views() {
    let ctx = member_ctx(admitted(&[]));
    let member = MemberHost::new(&ctx, CommandDescriptor::NONE);
    let resolved = futures::executor::block_on(member.resolved()).expect("the kernel resolves");

    let migrations = resolved
        .get::<AdmittedMigrations>()
        .expect("the migrations view is open");
    assert!(
        migrations
            .members()
            .iter()
            .any(|(meta, _)| meta.id == PROBE)
    );
    assert!(resolved.get::<AdmittedConfigs>().is_ok());
    assert!(resolved.get::<AdmittedAssets>().is_ok());
}

#[test]
fn the_admitted_views_carry_the_marketplace_doctor_check_and_no_other_owners() {
    let admitted = systemprompt_test_fixtures::fixture_admitted();
    let views = systemprompt_extension::cli::AdmittedViews::of(&admitted);
    let checks: Vec<(&str, &str)> = views
        .doctor
        .entries()
        .iter()
        .map(|owned| (owned.owner.as_str(), owned.check.name()))
        .collect();
    assert!(
        checks.contains(&("managed_resources", "git-credentials")),
        "{checks:?}"
    );

    let without = systemprompt_test_fixtures::fixture_admitted_without(&[
        systemprompt_identifiers::ExtensionId::new("managed_resources"),
    ]);
    let views = systemprompt_extension::cli::AdmittedViews::of(&without);
    assert!(
        views
            .doctor
            .entries()
            .iter()
            .all(|owned| owned.owner.as_str() != "managed_resources"),
        "a member outside the composition contributes no doctor check"
    );
}
