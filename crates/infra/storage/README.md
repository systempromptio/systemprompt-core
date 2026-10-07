# systemprompt-storage

Vendor-agnostic file storage for systemprompt.io. Provides the local-disk
and Google Cloud Storage implementations of the `FileStorage` trait, built
through `build_file_storage(FileStorageBackend::{Local, Gcs})`, and the
shared-mount probe that
lets a multi-replica deployment confirm every node sees the same storage
root.
