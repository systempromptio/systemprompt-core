use systemprompt_cli::kernel::MemberHost;
use systemprompt_extension::cli::{CommandDescriptor, CommandHost, KernelGrant};

use crate::boot_ctx::bootstrap_ctx;

fn refused_for(error: &impl std::fmt::Display, grant: KernelGrant) -> bool {
    error
        .to_string()
        .contains(&format!("`{grant}` kernel grant"))
}

#[test]
fn a_member_command_reads_the_profile_only_under_boot_profile() {
    let ctx = bootstrap_ctx();
    let error = MemberHost::new(&ctx, CommandDescriptor::FULL)
        .profile()
        .expect_err("no grant, no profile");
    assert!(refused_for(&error, KernelGrant::BootProfile), "{error}");
    MemberHost::new(
        &ctx,
        CommandDescriptor::FULL.with_grant(KernelGrant::BootProfile),
    )
    .profile()
    .expect("negative control: boot-profile serves the profile");
}

#[test]
fn a_member_command_reads_the_services_tree_only_under_its_grant() {
    let ctx = bootstrap_ctx();
    let error = MemberHost::new(&ctx, CommandDescriptor::PROFILE_SECRETS_AND_PATHS)
        .services_tree()
        .expect_err("no grant, no services tree");
    assert!(refused_for(&error, KernelGrant::ServicesTree), "{error}");

    let tree = MemberHost::new(
        &ctx,
        CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_grant(KernelGrant::ServicesTree),
    )
    .services_tree()
    .expect("services-tree serves the tree without a database");
    let profile = ctx.profile().expect("the bootstrap profile");
    let root = ctx.boot().expect("boot").require_services_root().path;
    assert_eq!(tree.root(), root.as_path());
    assert_eq!(tree.plugins(), root.join("plugins"));
    assert_eq!(tree.api_external_url(), profile.server.api_external_url);
    assert_eq!(tree.site_github_link(), profile.site.github_link.as_deref());
}

#[test]
fn a_member_command_reads_the_exports_directory_only_under_local_export() {
    let ctx = bootstrap_ctx();
    let error = MemberHost::new(&ctx, CommandDescriptor::FULL)
        .export_dir()
        .expect_err("no grant, no exports directory");
    assert!(refused_for(&error, KernelGrant::LocalExport), "{error}");
    MemberHost::new(
        &ctx,
        CommandDescriptor::FULL.with_grant(KernelGrant::LocalExport),
    )
    .export_dir()
    .expect("negative control: local-export serves the exports directory");
}

#[tokio::test]
async fn a_member_command_is_refused_a_session_without_its_grant() {
    let ctx = bootstrap_ctx();
    let error = MemberHost::new(&ctx, CommandDescriptor::FULL)
        .caller_session()
        .await
        .expect_err("no grant, no session");
    assert!(refused_for(&error, KernelGrant::CallerSession), "{error}");
}

#[test]
fn the_kernel_context_hands_out_no_granted_value_on_its_command_face() {
    let ctx = bootstrap_ctx();
    let host: &dyn CommandHost = &ctx;
    assert!(refused_for(
        &host.services_tree().expect_err("refused"),
        KernelGrant::ServicesTree
    ));
    assert!(refused_for(
        &host.export_dir().expect_err("refused"),
        KernelGrant::LocalExport
    ));
}
