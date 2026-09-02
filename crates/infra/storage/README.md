# systemprompt-storage

Vendor-agnostic file storage for systemprompt.io. Provides the local-disk
implementation of the `FileStorage` trait and the shared-mount probe that
lets a multi-replica deployment confirm every node sees the same storage
root.
