//! Agent access to documents: who is acting, how far an agent may go on a
//! document, and the seam an extension sets that policy through.
//!
//! A call into the document service runs as a [`DocumentActor`]: a person
//! acting directly, or an agent acting for them. A person is never capped.
//! An agent's effective level is the lower of the user's own level and the
//! document's [`AgentCap`]: `none` hides the document from the agent
//! entirely (listings, search and the drive feed drop it, a direct read is
//! `not_found`), `read` and `comment` cap the level, `suggest` turns every
//! edit into a proposal a person accepts, and `edit` leaves the user's level
//! as it is.
//!
//! The cap is the stricter of two sources: the registered
//! [`AgentAccessPolicy`] (core's default answers `edit` for every document,
//! today's behaviour) and an owner-set override on the document or any
//! folder above it. An override only ever tightens; one looser than the
//! policy is refused.
//!
//! Whatever the cap, an agent may not take the [`AgentCapability`] actions
//! (share or change grants, grant access through a mention, trash, delete,
//! restore, replace a document's whole content, edit a template) unless the
//! policy allows that capability; core's default allows none of them.
//!
//! The actor is set once per call with [`act_as`] by the surface that knows
//! the credential; code that never sets it acts as an unnamed, unattended
//! agent, so a missed scope fails closed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod actor;
mod cap;
mod error;
mod policy;
mod rate;

pub use actor::{
    AgentActor, DocumentActor, UNNAMED_AGENT, act_as, current_actor, set_unscoped_actor,
    unscoped_actor,
};
pub use cap::{AgentCap, AgentCapability, CapSource, EffectiveCap};
pub use error::AgentRefusal;
pub use policy::{
    AgentAccessPolicy, AgentAccessPolicyRegistration, DefaultAgentAccessPolicy, DocumentContext,
    MoveActor, registered_policy,
};
pub use rate::AgentRateLimiter;
