# Changelog

## [0.76.0] - 2026-10-29

### Breaking

- **Breaking:** `rich::read` returns `Result<RichText, rich::ReadError>` and fails on a block whose id or kind cannot be read instead of leaving it out. Migrate by handling the error.

### Fixed

- A server-side write to a live document reuses the document's peer id instead of minting a new peer per write.
- Writing a model into a document matches blocks through an id index and reads each block's text once, so a write of `n` blocks is `O(n log n)`.
- `check_update` refuses from the encoded header alone an oversized, non-incremental or inconsistent update, or one carrying operations of an unassigned peer; `apply_update` reports the peers an update added and whether part of it is pending.

## [0.75.0] - 2026-10-28

### Added

- `LoroEngine`, the Loro 1.17.1 implementation of the files domain's `DocumentEngine`, moved out of `systemprompt-files`; `WIRE_VERSION` names the wire format a client must speak.
- The `RichText` model in Loro containers (`rich`, `write_model`, `seed_rich`, `TEXT_STYLES`) and `EngineDoc::fork`.
