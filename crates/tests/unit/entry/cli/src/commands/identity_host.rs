//! Runs the identity member's command groups against a test context through
//! the member host, with the grants their reserved paths list.

use clap::Parser;
use systemprompt_cli::CommandContext;
use systemprompt_cli::kernel::MemberHost;
use systemprompt_extension::cli::{CommandOutput, DescribeCommand, GrantedPlane, KernelGrant};
use systemprompt_identity_entry::cli::bridge::{self, BridgeCommands};
use systemprompt_identity_entry::cli::keys::{self, KeysCommands};
use systemprompt_identity_entry::cli::users::{self, UsersCommands};

#[derive(Debug, Parser)]
struct UsersHarness {
    #[command(subcommand)]
    cmd: UsersCommands,
}

pub fn parse_users(args: &[&str]) -> UsersCommands {
    UsersHarness::try_parse_from(std::iter::once("users").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

pub async fn users(cmd: UsersCommands, ctx: &CommandContext) -> anyhow::Result<CommandOutput> {
    let descriptor = cmd
        .descriptor()
        .with_grant(KernelGrant::ConfigPlaneWriter(GrantedPlane::Identity));
    users::execute(cmd, &MemberHost::new(ctx, descriptor)).await
}

pub async fn bridge(cmd: BridgeCommands, ctx: &CommandContext) -> anyhow::Result<CommandOutput> {
    let descriptor = cmd.descriptor().with_grant(KernelGrant::SigningKeys);
    bridge::execute(cmd, &MemberHost::new(ctx, descriptor)).await
}

pub async fn keys(cmd: KeysCommands, ctx: &CommandContext) -> anyhow::Result<CommandOutput> {
    let descriptor = cmd.descriptor().with_grant(KernelGrant::SigningKeys);
    keys::execute(cmd, &MemberHost::new(ctx, descriptor)).await
}
