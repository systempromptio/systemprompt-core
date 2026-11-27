//! The scripted instance against the linked core crates: the real device
//! client signs in and syncs against it, a manifest it signs verifies with
//! the device's verifier, and a stream it encodes decodes with the wire
//! crate's decoders to the scripted turn.

mod device;
mod faults;
mod hooks;
mod manifest;
mod streams;
mod support;
