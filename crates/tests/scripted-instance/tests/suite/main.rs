//! The scripted instance against the linked core crates: the real device
//! client signs in and syncs against it, a manifest it signs verifies with
//! the device's verifier, and a stream it encodes decodes with the wire
//! crate's decoders to the scripted turn.

mod asides;
mod bundles;
mod catalog;
mod consumer;
mod device;
mod drive;
mod faults;
mod feed;
mod gaps;
mod hooks;
mod manifest;
mod plans;
mod streams;
mod support;
