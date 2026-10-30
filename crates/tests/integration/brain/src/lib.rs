//! Database-backed tests of the brain: outbox capture into revisioned facts,
//! the lifecycle service, the declared plane, recall with source checks, and
//! the review and retention jobs. Each test owns a disposable database.

#[cfg(test)]
mod consumer_drain;
#[cfg(test)]
mod dedupe_dispute;
#[cfg(test)]
mod disabled;
#[cfg(test)]
mod events;
#[cfg(test)]
mod extraction;
#[cfg(test)]
mod fakes;
#[cfg(test)]
mod harness;
#[cfg(test)]
mod jobs_support;
#[cfg(test)]
mod lifecycle;
#[cfg(test)]
mod no_filter;
#[cfg(test)]
mod outbox_to_fact;
#[cfg(test)]
mod plane;
#[cfg(test)]
mod plane_round_trip;
#[cfg(test)]
mod proposals;
#[cfg(test)]
mod purge;
#[cfg(test)]
mod read_visibility;
#[cfg(test)]
mod recall_seed;
#[cfg(test)]
mod recall_support;
#[cfg(test)]
mod recall_tiers;
#[cfg(test)]
mod retention;
#[cfg(test)]
mod review_after;
#[cfg(test)]
mod review_job;
#[cfg(test)]
mod stale;
#[cfg(test)]
mod withheld;
