# Changelog

## [0.77.0] - 2026-10-30

### Added

- Initial release: the `brain_*` schema for declared kinds, topics, interests, workflow profiles and config documents, subjects with trigram-matched aliases, facts with append-only revisions, capture ledgers, recalls, outcomes and proposals.
- `BrainRepositories` over every brain table, with `FactRepository::apply` as the single path that changes a stored fact and records its revision.
- `OutboxBrainSink`, which appends brain events to the durable outbox on the producer's connection.
- `BrainExtension`, which registers the schema and migrations and enrols personal facts, recalls and interests in the user purge.
