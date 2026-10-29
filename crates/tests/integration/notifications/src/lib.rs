//! Database-backed tests of the notifications pipeline and service. Each test
//! owns a disposable database, so it alone produces and drains the
//! `notifications` outbox consumer's facts.

#[cfg(test)]
mod audience;
#[cfg(test)]
mod coalescing;
#[cfg(test)]
mod consumer;
#[cfg(test)]
mod delivery;
#[cfg(test)]
mod digest;
#[cfg(test)]
mod escalation;
#[cfg(test)]
mod harness;
#[cfg(test)]
mod mentions;
#[cfg(test)]
mod ownership;
#[cfg(test)]
mod policy;
#[cfg(test)]
mod receipts;
#[cfg(test)]
mod retention;
#[cfg(test)]
mod simulate;
#[cfg(test)]
mod streams;
#[cfg(test)]
mod visibility;
