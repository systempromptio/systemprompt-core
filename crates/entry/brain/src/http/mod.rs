//! `/api/v1/brain` and `/api/v1/admin/brain` over HTTP.
//!
//! The user routes search, read, enter and govern facts, resolve disputes
//! and decide proposals; the admin route simulates a recall for a chosen
//! principal without recording it. [`user_routes`] and [`admin_routes`] are
//! the two route sets [`crate::extension::BrainHttpExtension`] contributes.
//! [`OpenBrain`], the first extractor of every handler of both groups, reads
//! the caller's current roles from the governance role directory: a caller
//! whose account is not active holds none and is refused with 403
//! `account_inactive` on every request, with no cache window. Each user
//! handler then calls one operation of the brain service, which authorizes
//! it. While the brain handle is unbound (the profile disables the brain)
//! every route answers 404 `brain_disabled`. Both refusals come before any
//! path, query or body is read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod admin;
mod dto;
mod error;
mod facts;
mod lifecycle;
mod mount;
mod proposals;

use std::ops::Deref;
use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use systemprompt_brain::service::{BrainCaller, BrainService};
use systemprompt_brain::{BrainHandle, BrainHandleSlot};
use systemprompt_extension::brain::MemoryRecall;
use systemprompt_extension::{FromState, MissingDependency, Resolved};
use systemprompt_governance::authz::{SharedRoleDirectory, SubjectProviderSet};
use systemprompt_governance::contract::RoleDirectories;
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::caller_actor;

pub use dto::{
    DisputeBody, DisputeCreated, EnterAccepted, FieldsBody, LimitParams, ResolveBody,
    ResolveOutcome, RetractBody, SearchParams, SimulateBody, SimulateResponse, WhyParams,
};
pub use error::BrainHttpError;
pub use mount::{admin_routes, user_routes};

/// Router-scoped state of the brain routes.
///
/// It holds the brain handle the slot build returned, the recall the admin
/// simulator runs, the brain profile, the role directory a caller's current
/// roles are read from and the subject providers.
#[derive(Clone)]
pub struct BrainHttpState {
    brain: Arc<BrainHandleSlot>,
    recall: Arc<dyn MemoryRecall>,
    profile: Arc<BrainProfile>,
    roles: SharedRoleDirectory,
    subject_providers: Arc<SubjectProviderSet>,
}

impl std::fmt::Debug for BrainHttpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrainHttpState")
            .field("brain", &self.brain)
            .finish_non_exhaustive()
    }
}

impl FromState for BrainHttpState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let roles = r
            .get::<Arc<RoleDirectories>>()?
            .directory()
            .ok_or_else(MissingDependency::of::<SharedRoleDirectory>)?;
        Ok(Self {
            brain: r.get()?,
            recall: r.get()?,
            profile: r.get()?,
            roles,
            subject_providers: r.get()?,
        })
    }
}

impl BrainHttpState {
    fn handle(&self) -> Result<&Arc<BrainHandle>, BrainHttpError> {
        self.brain.get().ok_or_else(|| {
            ApiError::not_found("the brain is not enabled")
                .with_error_key("brain_disabled")
                .into()
        })
    }

    async fn active_roles(&self, req_ctx: &RequestContext) -> Result<Vec<String>, BrainHttpError> {
        self.roles
            .roles_of(req_ctx.user_id())
            .await?
            .ok_or_else(|| {
                ApiError::forbidden("the caller's account is not active")
                    .with_error_key("account_inactive")
                    .into()
            })
    }
}

/// The brain route state of a request from an active caller.
///
/// Extracting it refuses with 404 `brain_disabled` while the brain handle is
/// unbound and with 403 `account_inactive` when the role directory holds no
/// active account for the caller.
#[derive(Debug, Clone)]
pub struct OpenBrain {
    state: BrainHttpState,
    roles: Vec<String>,
}

impl Deref for OpenBrain {
    type Target = BrainHttpState;

    fn deref(&self) -> &BrainHttpState {
        &self.state
    }
}

impl OpenBrain {
    fn open(
        &self,
        req_ctx: &RequestContext,
    ) -> Result<(Arc<BrainService>, BrainCaller), BrainHttpError> {
        let service = Arc::clone(self.state.handle()?.service());
        let caller = BrainCaller::new(req_ctx.user_id().clone(), self.roles.clone())
            .with_actor(caller_actor(req_ctx));
        Ok((service, caller))
    }
}

impl FromRequestParts<BrainHttpState> for OpenBrain {
    type Rejection = BrainHttpError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &BrainHttpState,
    ) -> Result<Self, Self::Rejection> {
        state.handle()?;
        let req_ctx = parts
            .extensions
            .get::<RequestContext>()
            .ok_or_else(|| BrainHttpError::from(ApiError::unauthorized("no verified caller")))?;
        let roles = state.active_roles(req_ctx).await?;
        Ok(Self {
            state: state.clone(),
            roles,
        })
    }
}
