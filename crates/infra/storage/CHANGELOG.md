# Changelog

## [0.44.0] - 2026-09-02

### Added

- New crate. A `FileStorage` implementation over a configurable root, plus a boot-time probe that writes a per-instance marker and warns when the profile's `shared` flag disagrees with what it finds on disk. Uploads and generated images go through the trait, so a shared mount works unchanged and an object-store backend has a seam to land in.

## 0.43.0

- Initial release: `LocalFileStorage`, `build_file_storage`, and
  `probe_shared_mount`.
