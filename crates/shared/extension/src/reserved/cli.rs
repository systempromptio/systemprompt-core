//! The reserved CLI paths: each held by one core member, with the grants it
//! may request and the claims its commands may make.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CoreId;
use crate::cli::{CommandClaim, KernelGrant};

/// A CLI path only its core owner may claim, with the grants it may request
/// and the claims its commands may make.
#[derive(Debug, Clone, Copy)]
pub struct ReservedCliPath {
    pub owner: CoreId,
    pub path: &'static [&'static str],
    pub grants: &'static [KernelGrant],
    pub claims: &'static [CommandClaim],
}

const fn cli(
    owner: CoreId,
    path: &'static [&'static str],
    grants: &'static [KernelGrant],
    claims: &'static [CommandClaim],
) -> ReservedCliPath {
    ReservedCliPath {
        owner,
        path,
        grants,
        claims,
    }
}

const DATABASE_READS: &[CommandClaim] = &[
    CommandClaim::ReadOnlyRouting,
    CommandClaim::PreservesData,
    CommandClaim::DirectDatabase,
];
const DIRECT_PRESERVING: &[CommandClaim] =
    &[CommandClaim::PreservesData, CommandClaim::DirectDatabase];
const PRESERVING: &[CommandClaim] = &[CommandClaim::PreservesData];

pub const RESERVED_CLI_PATHS: &[ReservedCliPath] = &[
    cli(
        CoreId::Kernel,
        &["admin", "mcp"],
        &[KernelGrant::ChildHost],
        &[],
    ),
    cli(
        CoreId::Kernel,
        &["admin", "agent-host"],
        &[KernelGrant::ChildHost],
        &[],
    ),
    cli(
        CoreId::AgentHttp,
        &["admin", "agents"],
        &[KernelGrant::ServicesTree, KernelGrant::CallerSession],
        PRESERVING,
    ),
    cli(
        CoreId::AgentHttp,
        &["core", "contexts"],
        &[KernelGrant::CallerSession, KernelGrant::SessionContext],
        PRESERVING,
    ),
    cli(
        CoreId::AgentHttp,
        &["core", "artifacts"],
        &[KernelGrant::CallerSession],
        PRESERVING,
    ),
    cli(
        CoreId::AnalyticsHttp,
        &["analytics"],
        &[KernelGrant::LocalExport, KernelGrant::SkipValidation],
        DATABASE_READS,
    ),
    cli(
        CoreId::AnalyticsHttp,
        &["core", "content", "popular"],
        &[],
        DIRECT_PRESERVING,
    ),
    cli(
        CoreId::ContentHttp,
        &["core", "content"],
        &[],
        DIRECT_PRESERVING,
    ),
    cli(
        CoreId::Cloud,
        &["cloud"],
        &[
            KernelGrant::CloudOperator,
            KernelGrant::BootProfile,
            KernelGrant::ConfigReconcile,
            KernelGrant::TargetAdmission,
        ],
        &[CommandClaim::ReadOnlyRouting, CommandClaim::PreservesData],
    ),
    cli(
        CoreId::MarketplaceHttp,
        &["core", "marketplace"],
        &[KernelGrant::ServicesTree],
        PRESERVING,
    ),
    cli(
        CoreId::MarketplaceHttp,
        &["core", "plugins"],
        &[KernelGrant::ServicesTree],
        &[],
    ),
    cli(
        CoreId::MarketplaceHttp,
        &["core", "skills"],
        &[KernelGrant::ServicesTree],
        &[CommandClaim::LocalDatabase, CommandClaim::PreservesData],
    ),
    cli(
        CoreId::FilesHttp,
        &["core", "files"],
        &[],
        DIRECT_PRESERVING,
    ),
    cli(
        CoreId::McpHttp,
        &["plugins", "mcp"],
        &[KernelGrant::ServicesTree, KernelGrant::CallerSession],
        PRESERVING,
    ),
];
