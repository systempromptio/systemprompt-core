//! The boxed factory and shared value types a member's sink stores.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_provider_contracts::DoctorCheck;
use systemprompt_provider_contracts::governance::GovernancePolicy;
use systemprompt_traits::RunnerService;
use systemprompt_traits::authz::SubjectAttributeProvider;

use super::resolved::Factory;
use crate::asset::{AssetDefinition, AssetPaths};
use crate::gateway_guard::GatewayRequestGuard;
use crate::health::{DiagnosticProbe, ReadinessCheck};
use crate::notifications::NotificationAudienceResolver;

pub type AssetFn = Arc<dyn Fn(&dyn AssetPaths) -> Vec<AssetDefinition> + Send + Sync>;
pub type RunnerServiceFactory = Box<dyn Factory<Arc<dyn RunnerService>>>;
pub type ReadinessFactory = Box<dyn Factory<Arc<dyn ReadinessCheck>>>;
pub type DiagnosticFactory = Box<dyn Factory<Arc<dyn DiagnosticProbe>>>;
pub type GatewayGuardFactory = Box<dyn Factory<Arc<dyn GatewayRequestGuard>>>;
pub type SubjectProviderFactory = Box<dyn Factory<Arc<dyn SubjectAttributeProvider>>>;
pub type PolicyFactory = Box<dyn Factory<Arc<dyn GovernancePolicy>>>;
pub type AudienceResolverFactory = Box<dyn Factory<Arc<dyn NotificationAudienceResolver>>>;
pub type DoctorCheckRef = Arc<dyn DoctorCheck>;
