//! The gateway read side: [`BrainRecallStore`] reads the scope-filtered
//! retrieval tiers and records recalls, and [`BrainRecallService`] turns a
//! recall request into the `<team_memory>` block.
//!
//! The pure stages are public so they can be exercised alone:
//! [`held`] (the working-set keys a principal holds), [`admit`] (the tier
//! SQL's admission rule over an introduced fact), [`synonyms`]
//! (query-time expansion), [`fusion`] (reciprocal rank fusion),
//! [`visible`] (the per-fact visibility decision), [`score`] (in-context
//! importance), [`rerank`] (the reranker's merge rule), [`budget`] (quota
//! and token fill), [`limits`] (the tiers' row limits), [`signal`] (the kinds a
//! message asks and the names it carries) and [`reuse`] (same-turn block
//! reuse). [`declared`] supplies the plane declarations recall and the
//! lifecycle services read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admit;
pub mod budget;
mod builder;
pub mod candidate;
pub mod declared;
pub mod fusion;
pub mod held;
pub mod introduce;
pub mod lifecycle;
pub mod limits;
mod new_turn;
pub mod rerank;
pub mod reuse;
pub mod score;
pub mod search_plan;
mod service;
pub mod signal;
mod store;
mod store_receipt;
mod store_tiers;
pub mod synonyms;
pub mod tiers;
pub mod visible;
mod visible_memo;

pub use builder::BrainRecallServiceBuilder;
pub use declared::{DeclaredRecall, DeclaredSource, PlaneDeclaredSource};
pub use held::HeldKeys;
pub use introduce::FactLoader;
pub use service::{BrainRecallService, STALE_MARKER, fact_text};
pub use store::BrainRecallStore;
pub use store_receipt::new_recall;
