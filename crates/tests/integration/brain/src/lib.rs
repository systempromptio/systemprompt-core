//! Database-backed tests of the brain: outbox capture into revisioned facts,
//! the lifecycle service, the declared plane, recall with source checks, and
//! the review and retention jobs. Each test owns a disposable database.

#[cfg(test)]
mod analysis_job;
#[cfg(test)]
mod analysis_store;
#[cfg(test)]
mod consumer_batch;
#[cfg(test)]
mod consumer_drain;
#[cfg(test)]
mod consumer_reindex;
#[cfg(test)]
mod dedupe_dispute;
#[cfg(test)]
mod dedupe_unchanged;
#[cfg(test)]
mod disabled;
#[cfg(test)]
mod disputes;
#[cfg(test)]
mod entry_and_edit;
#[cfg(test)]
mod events;
#[cfg(test)]
mod extraction;
#[cfg(test)]
mod fact_index;
#[cfg(test)]
mod fakes;
#[cfg(test)]
mod harness;
#[cfg(test)]
mod index_backlog;
#[cfg(test)]
mod jobs_support;
#[cfg(test)]
mod lifecycle;
#[cfg(test)]
mod mcp_read_tools;
#[cfg(test)]
mod no_filter;
#[cfg(test)]
mod outbox_to_fact;
#[cfg(test)]
mod plane;
#[cfg(test)]
mod plane_interest_owner;
#[cfg(test)]
mod plane_round_trip;
#[cfg(test)]
mod proposals;
#[cfg(test)]
mod purge;
#[cfg(test)]
mod read_visibility;
#[cfg(test)]
mod recall_determinism;
#[cfg(test)]
mod recall_extension;
#[cfg(test)]
mod recall_extension_admit;
#[cfg(test)]
mod recall_seed;
#[cfg(test)]
mod recall_stale_marker;
#[cfg(test)]
mod recall_support;
#[cfg(test)]
mod recall_tiers;
#[cfg(test)]
mod recall_withheld_as_of;
#[cfg(test)]
mod recall_working_set;
#[cfg(test)]
mod replay_window;
#[cfg(test)]
mod retention;
#[cfg(test)]
mod review_after;
#[cfg(test)]
mod review_job;
#[cfg(test)]
mod round_tombstone;
#[cfg(test)]
mod seams_discovery;
#[cfg(test)]
mod search_filter;
#[cfg(test)]
mod service_builder;
#[cfg(test)]
mod service_reads;
#[cfg(test)]
mod session_pathway;
#[cfg(test)]
mod stale;
#[cfg(test)]
mod subjects_aliases;
#[cfg(test)]
mod usage_job;
#[cfg(test)]
mod withheld;
#[cfg(test)]
mod working_set;
