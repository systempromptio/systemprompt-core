//! Golden catalog recorder: what a catalog resolves every requested model id
//! to, in the schema of the committed 0.72.1 snapshots, kept free of
//! database-backed fixtures so it builds offline.

#[cfg(test)]
mod golden_catalog;
