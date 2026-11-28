//! Behaviour tests for the engine's model clients, migrated case by case from
//! the reference suite (batch 3 of the parity programme): request bodies on
//! both wire protocols, stream decoding, tool-call assembly, error
//! classification and the HTTP transport. Each test states one behaviour in
//! the harness's own terms; the link from a test to the case it carries lives
//! in `cases.md` beside this crate, never in the test itself. A test marked
//! `#[ignore = "parity ledger: …"]` asserts the reference behaviour where the
//! harness differs.

#[cfg(test)]
mod chat_body_tests;
#[cfg(test)]
mod chat_stream_tests;
#[cfg(test)]
mod errors_tests;
#[cfg(test)]
mod messages_body_tests;
#[cfg(test)]
mod messages_stream_tests;
#[cfg(test)]
mod scripted_tests;
#[cfg(test)]
mod sse_tests;
#[cfg(test)]
mod support;
#[cfg(test)]
mod tool_input_tests;
#[cfg(test)]
mod transport_tests;
