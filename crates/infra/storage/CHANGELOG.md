# Changelog

## [Unreleased]

### Fixed

- `probe_shared_mount` no longer leaks a marker per process when the instance id is random (a local profile with no `instance_id` and no `HOSTNAME`). The process removes its own marker after the read-back check, and markers left by earlier random-id processes are pruned instead of being reported as sibling replicas, which made every local boot warn that `storage.shared` was wrong.

## [0.53.0] - 2026-09-15

### Fixed

- `LocalFileStorage::store` writes to a per-process staging file and renames it into place, so a reader on another replica of a shared mount never sees a truncated file.

## [0.44.0] - 2026-09-02

### Added

- New crate. A `FileStorage` implementation over a configurable root, plus a boot-time probe that writes a per-instance marker and warns when the profile's `shared` flag disagrees with what it finds on disk. Uploads and generated images go through the trait, so a shared mount works unchanged and an object-store backend has a seam to land in.

## 0.43.0

- Initial release: `LocalFileStorage`, `build_file_storage`, and
  `probe_shared_mount`.
