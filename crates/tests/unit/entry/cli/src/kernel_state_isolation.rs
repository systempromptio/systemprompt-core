//! The boot-only values a member's command must not resolve, and the
//! profile boot a member reaches only with the `boot-profile` grant; sealed
//! seeds a member's resolver refuses and a kernel command reads; the config
//! apply service only a `config-reconcile` grant resolves.

use std::sync::Arc;

use systemprompt_cli::kernel::{KernelHost, MemberHost};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_config::SecretsSources;
use systemprompt_extension::cli::{
    CliConfig, CommandDescriptor, CommandHost, KernelGrant, OutputFormat,
};
use systemprompt_identifiers::{InstanceId, UserId};
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_runtime::Kernel;
use systemprompt_traits::{BackgroundTasks, Dependencies};

use crate::kernel_host::profile_host;
use crate::services_profile_fixture as fx;

fn assert_missing<T: Clone + Send + Sync + 'static>(
    host: &dyn CommandHost,
    runtime: &tokio::runtime::Runtime,
) {
    runtime.block_on(async {
        let resolved = host.resolved().await.expect("the detached kernel resolves");
        assert!(
            resolved.get::<T>().is_err(),
            "{} reached a member's resolved state",
            std::any::type_name::<T>()
        );
    });
}

#[test]
fn a_member_command_resolves_none_of_the_boot_only_values() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let ctx = profile_host(&tree.profile_path);
    let member = MemberHost::new(&ctx, CommandDescriptor::NONE);

    assert_missing::<SecretsSources>(&member, &runtime);
    assert_missing::<EnvOverrides>(&member, &runtime);
    assert_missing::<Arc<Config>>(&member, &runtime);
    assert_missing::<Arc<systemprompt_loader::BootContext>>(&member, &runtime);
    assert_missing::<Arc<systemprompt_runtime::AppContext>>(&member, &runtime);
}

#[test]
fn a_kernel_command_reads_the_boot_only_values_from_its_host() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let ctx = profile_host(&tree.profile_path);
    let host: &dyn KernelHost = &ctx;

    assert!(host.env_overrides().profile.is_none());
    let _sources: &SecretsSources = host.secrets_sources();
}

#[tokio::test]
async fn an_ungranted_member_command_cannot_boot_a_profile() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let ctx = profile_host(&tree.profile_path);
    let member = MemberHost::new(&ctx, CommandDescriptor::NONE);

    let err = member
        .boot_config(&tree.profile_path)
        .await
        .expect_err("no boot-profile grant");
    assert!(err.to_string().contains("boot-profile"), "{err}");

    let err = CommandHost::boot_config(&ctx, &tree.profile_path)
        .await
        .expect_err("the context's member face never boots a profile");
    assert!(err.to_string().contains("boot-profile"), "{err}");
}

#[tokio::test]
async fn a_granted_member_command_reaches_the_profile_boot() {
    let tree = fx::write_tree(&fx::https_sources_block(&[]), "");
    let ctx = profile_host(&tree.profile_path);
    let member = MemberHost::new(
        &ctx,
        CommandDescriptor::NONE.with_grant(KernelGrant::BootProfile),
    );

    if let Err(err) = member.boot_config(&tree.profile_path).await {
        assert!(!err.to_string().contains("boot-profile"), "{err}");
    }
}

fn sealed_host() -> CommandContext {
    let seed = Dependencies::new()
        .with_sealed(SystemAdmin::new(UserId::new("admin-1"), "admin".to_owned()))
        .with(InstanceId::new("instance-1"));
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_kernel(Arc::new(Kernel::detached(BackgroundTasks::new(), seed)))
}

#[tokio::test]
async fn a_member_command_is_refused_a_sealed_seed_as_missing() {
    let ctx = sealed_host();
    let member = MemberHost::new(&ctx, CommandDescriptor::NONE);
    let resolved = member.resolved().await.expect("the kernel resolves");

    let missing = resolved.get::<SystemAdmin>().expect_err("sealed");
    assert_eq!(
        missing,
        systemprompt_extension::MissingDependency::of::<SystemAdmin>()
    );
    assert!(resolved.optional::<SystemAdmin>().is_none());
    assert!(
        resolved.get::<InstanceId>().is_ok(),
        "an open seed is served"
    );
}

#[tokio::test]
async fn a_kernel_command_reads_a_sealed_seed() {
    let ctx = sealed_host();
    let host: &dyn KernelHost = &ctx;
    let resolved = host.resolved().await.expect("the kernel resolves");

    assert!(resolved.get::<SystemAdmin>().is_ok());
}

async fn app_host() -> CommandContext {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let url = systemprompt_test_fixtures::test_database_url();
    crate::boot_ctx::with_app_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        systemprompt_test_fixtures::test_app_context(&url).await,
    )
}

#[tokio::test]
async fn a_config_reconcile_grant_serves_the_config_apply_service() {
    let ctx = app_host().await;
    let member = MemberHost::new(
        &ctx,
        CommandDescriptor::NONE.with_grant(KernelGrant::ConfigReconcile),
    );
    let resolved = member.resolved().await.expect("the granted state resolves");

    assert!(
        resolved
            .get::<Arc<systemprompt_scheduler::services::ConfigApplyService>>()
            .is_ok()
    );
    assert!(
        resolved
            .get::<Arc<systemprompt_runtime::AppContext>>()
            .is_err(),
        "no member resolves the application context"
    );
    assert!(
        resolved.get::<Arc<Config>>().is_err(),
        "the grant unseals nothing"
    );
}

#[tokio::test]
async fn an_ungranted_member_is_refused_the_config_apply_service() {
    let ctx = app_host().await;
    let member = MemberHost::new(&ctx, CommandDescriptor::NONE);
    let resolved = member.resolved().await.expect("the kernel resolves");

    assert!(
        resolved
            .get::<Arc<systemprompt_scheduler::services::ConfigApplyService>>()
            .is_err()
    );
}
