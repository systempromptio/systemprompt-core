//! The identity member's kernel grants: a member command that does not hold
//! `SigningKeys` or the identity `ConfigPlaneWriter` is refused the signing
//! handle and the plane check with a typed grant refusal, and the kernel's own
//! command context serves neither to a member.

use std::sync::Arc;

use systemprompt_cli::kernel::MemberHost;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::StandingTag;
use systemprompt_extension::cli::{
    CliConfig, CommandDescriptor, CommandHost, GrantedPlane, KernelGrant, OutputFormat,
};

use systemprompt_identifiers::ExtensionId;

use crate::probe::profile;

fn member_ctx() -> CommandContext {
    let admitted = systemprompt::reference()
        .build(&profile(&[]))
        .expect("the composition admits");
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
    .with_admitted(Some(Arc::new(admitted)))
    .with_detached_kernel()
}

fn refused_grant(error: &impl std::fmt::Display, grant: KernelGrant) -> bool {
    error.to_string() == format!("this command requires the `{grant}` kernel grant")
}

#[test]
fn a_member_command_without_the_signing_keys_grant_is_refused_the_handle() {
    let ctx = member_ctx();
    let descriptors = [
        CommandDescriptor::FULL,
        CommandDescriptor::FULL.with_grant(KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)),
        CommandDescriptor::FULL.with_grant(KernelGrant::CallerSession),
    ];
    for descriptor in descriptors {
        let member = MemberHost::new(&ctx, descriptor);
        let Err(error) = member.signing_keys() else {
            panic!("a command without the grant must not reach the signing keys");
        };
        assert!(refused_grant(&error, KernelGrant::SigningKeys), "{error}");
    }
}

#[test]
fn a_member_command_without_the_plane_grant_is_refused_the_plane() {
    let ctx = member_ctx();
    let member = MemberHost::new(
        &ctx,
        CommandDescriptor::FULL.with_grant(KernelGrant::SigningKeys),
    );
    let error = futures::executor::block_on(member.require_writable_plane(GrantedPlane::Identity))
        .expect_err("a command without the grant must not reach the plane");
    let grant = KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity);
    assert!(refused_grant(&error, grant), "{error}");
}

#[test]
fn the_kernel_context_serves_no_identity_grant() {
    let ctx = member_ctx();
    let Err(error) = CommandHost::signing_keys(&ctx) else {
        panic!("the kernel context hands no signing handle out");
    };
    assert!(refused_grant(&error, KernelGrant::SigningKeys), "{error}");
    let error = futures::executor::block_on(CommandHost::require_writable_plane(
        &ctx,
        GrantedPlane::Identity,
    ))
    .expect_err("the kernel context confirms no plane");
    assert!(
        refused_grant(
            &error,
            KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity)
        ),
        "{error}"
    );
}

#[test]
fn a_member_command_holding_the_signing_keys_grant_gets_the_handle() {
    let ctx = member_ctx();
    let member = MemberHost::new(
        &ctx,
        CommandDescriptor::FULL.with_grant(KernelGrant::SigningKeys),
    );
    let Err(error) = member.signing_keys() else {
        panic!("a context with no booted profile has no signing state to hand out");
    };
    assert!(!refused_grant(&error, KernelGrant::SigningKeys), "{error}");
}

const SETTINGS_REFUSED: &str = "only a command identity-http owns reads the identity settings";

#[test]
fn only_a_command_identity_http_owns_with_core_standing_reads_the_identity_settings() {
    let ctx = member_ctx();
    let refused = [
        MemberHost::new(&ctx, CommandDescriptor::FULL),
        MemberHost::new(&ctx, CommandDescriptor::FULL)
            .owned_by(ExtensionId::new("identity-http"), StandingTag::Install),
        MemberHost::new(&ctx, CommandDescriptor::FULL)
            .owned_by(ExtensionId::new("an-install"), StandingTag::Install),
        MemberHost::new(&ctx, CommandDescriptor::FULL)
            .owned_by(ExtensionId::new("gateway-http"), StandingTag::Core),
    ];
    for member in refused {
        let Err(error) = member.identity_settings() else {
            panic!("{member:?} must not read the identity settings");
        };
        assert_eq!(error.to_string(), SETTINGS_REFUSED, "{member:?}");
    }
    let Err(error) = CommandHost::identity_settings(&ctx) else {
        panic!("the kernel context serves no identity settings to a member");
    };
    assert_eq!(error.to_string(), SETTINGS_REFUSED);
    let owned = MemberHost::new(&ctx, CommandDescriptor::FULL)
        .owned_by(ExtensionId::new("identity-http"), StandingTag::Core);
    let Err(error) = owned.identity_settings() else {
        panic!("a context with no loaded profile has no configuration to read");
    };
    assert_ne!(error.to_string(), SETTINGS_REFUSED);
}
