# Changelog

## [0.77.0] - 2026-10-30

### Breaking

- **Breaking:** the document browser client is removed: the `sp-*` web components (`sp-document-browser`, `sp-document-preview`, `sp-presence-bar`, `sp-sheet-grid`, `sp-slide-text`, `sp-share-dialog`, `sp-subject-picker`, `sp-history`, `sp-comments`, `sp-proposal-list`), the `documents.js` entry, the `DocumentsElement` base class, the browser and share views, `assets/documents/css/*` and the headless client modules `documents-api.js`, `documents-events.js`, `documents-patch.js` and `documents-html.js`. Migrate by vendoring the document client and UI in the host extension.
- **Breaking:** `EditModeRequest` no longer has an `actor` field, and `DocumentService::edit_mode` reads the document actor from the request scope like every other document call. Migrate by deleting the field and wrapping the call in `act_as`.
- **Breaking:** `AgentAccessPolicy::cap` is a required method. Migrate by implementing it; `DefaultAgentAccessPolicy` keeps answering `edit`.
- **Breaking:** `DocumentAccessService::validate_subject` returns `Result<SubjectValidation, DocumentsError>`, where `Unenumerable` marks a dimension whose values cannot be listed. Migrate by matching the outcome and refusing `Invalid`.
- **Breaking:** `UploadError::Invalid(String)` is replaced by `InvalidName`, `EmptyUpload`, `InvalidContentType`, `SizeMismatch`, `PartSizeOutOfRange` and `PartHashUnavailable`, and the HTTP status and `invalid_request` key are unchanged. Migrate by matching the typed variants.
- **Breaking:** `DocumentService::admit_media` takes `&UploadService` instead of `Option<&UploadService>`, and `refuse_unheld_media` answers the no-uploads case. Migrate by calling `refuse_unheld_media(refs, &held)` where no `UploadService` is configured.
- **Breaking:** `AgentActor::name`, `via_agent` and `created_by_agent` are `AgentName`, and `DocumentActor::agent`, `AgentRateLimiter::reserve` and `RewriteTracker` take it; `holder_id`/`holder_kind` become `LockHolder`; `origin_id`/`origin_kind` become `RevisionOrigin` (now in `documents::models`); `storage_id` is `StoredFileId`. Migrate by constructing the typed values with `try_new`.
- **Breaking:** `DocumentServiceParts` gains a `brain` sink. Migrate by passing a `DynBrainSink` (the runtime's unavailable sink when the brain feature is off).
- **Breaking:** `DocumentsError` gains `Brain`, raised when the brain sink cannot record a `document.revision.landed` or `document.comment.resolved` fact; the landing or resolution rolls back. Migrate exhaustive matches by adding the variant.
- **Breaking:** `DocumentCommentRepository::resolve` and `set_answering` are replaced by `resolve_in` and `set_answering_in`, which run on the caller's connection. Migrate by calling them inside a transaction.

### Fixed

- `POST /documents/{id}/edits` applies a folder's agent edit policy to an agent bearer.
- A live document stream closes when its periodic access recheck fails, not only when access is withdrawn.
- An agent listing withholds a document whose folder chain cannot be read instead of treating it as having no overrides.
- The document routes refuse a request without an authenticated context with `401`, answer `500` when a browser-session request reaches them without the site configuration, and refuse a browser-session request carrying neither `Origin` nor `Sec-Fetch-Site`.
- `GET .../agent-cap` answers for an unattended agent unless `attended=true`, and validates `agent` as an `AgentName`.
- `GET /uploads/{id}/content` sends no `ETag` for an upload without a blob hash, so `If-None-Match` can no longer match an empty tag.
- A document image whose stored MIME type is not a valid header value is served as `application/octet-stream` instead of without a `Content-Type`.

### Removed

- **Removed:** **Engine (Loro):** the `systemprompt-engine-loro` crate; the Loro engine is `systemprompt-files` feature `collab-loro` at `documents::engine::loro`. Migrate by enabling `collab-loro` (or the facade's `files-collab-loro`) and importing from `systemprompt_files`.

### Changed

- blob collection asks the blob holders registered with `register_blob_holder!` instead of naming the issue attachment table, and the issue report user purge is registered by the issues module.
- `DocumentService::create_from_file`, `DocumentService::update_folder`, `DocumentService::open_pdf_stream`, `UploadService::retract`, `IssueService::ensure_attachable` and `IssueService::require_triager` carry rules previously inline in the API handlers; `IssuesError::TriageForbidden` is new.

## [0.76.0] - 2026-10-29

### Breaking

- **Breaking:** `EngineError` gains `PeerNotAssigned { peer }`, raised when an update carries changes under an engine peer the session did not assign to the caller. Migrate by adding an arm (or a wildcard) to exhaustive matches.
- **Breaking:** the Drive change feed cursor is a landing position (`DriveCursor`); `drive_changes` and `parse_drive_cursor` take and return it, `DocumentPathChange::cursor` is a `DriveCursor`, `ListCursorError` gains `Position`, and a cursor that is not a landing position is refused with `CursorReset`. Migrate Drive clients by restarting from a fresh manifest cursor when a cursor is reset.
- **Breaking:** `CHANGE_LAG` and `DocumentRevisionRepository::changes_since` are removed; `path_changes_since` takes a `DriveCursor` and a `FeedHorizon`. Migrate by reading the feed through `DocumentService::drive_changes`.
- **Breaking:** `EngineDoc::export_model` returns `Result<ModelView, EngineError>`, and `EngineError` gains `Model`, reported when an engine holds content that does not read as a model. Migrate by propagating the error from `export_model`.
- **Breaking:** `SubjectDirectory` gains `roles_of_many`, which answers the roles of several users in one lookup, in input order. Migrate custom directories by implementing it.
- **Breaking:** `DocumentsConfig` gains `agent_read_retention_days` (`files.documents.agentReadRetentionDays`, default `90`, at least `1`), and the new cluster job `document_agent_read_retention` deletes agent read audit rows older than it. Migrate struct literals by adding the field or `..DocumentsConfig::default()`.
- **Breaking:** `DocumentEditSessionRepository::set_checkpoint` takes a `CheckpointLease`, and the repository gains `claim_checkpoint`, `release_checkpoint`, `close_settled` and `list_stale`; `CollabError` gains `CheckpointBusy`, `CheckpointLeaseLost` and `ImageOwners`. Migrate by claiming a lease before recording a checkpoint and adding arms (or a wildcard) to exhaustive matches.
- **Breaking:** `DocumentLimits` gains `max_package_part_bytes` (`limits.maxPackagePartBytes`, default 64 MiB), `max_package_inflated_bytes` (`limits.maxPackageInflatedBytes`, default 256 MiB) and `max_package_parts` (`limits.maxPackageParts`, default 4,096), and `bounded()` returns twelve entries. Migrate struct literals by adding the fields or `..DocumentLimits::default()`.
- **Breaking:** `TemplateError::NotFound` refuses as `not_found` (404); `Io`, `NoCodec` and `Catalog` answer the fixed 5xx body, and their messages no longer carry a file path or the catalog cause. Migrate clients matching `template_failed` for an unknown template by matching `not_found`.
- **Breaking:** `UploadService::abort` returns an `ObjectRemoval` listing the objects whose delete failed, and `UploadService::expire` returns an `ExpiryOutcome`; `UploadService::sweep_orphans` and `DocumentUploadRepository::{track_object, untrack_object, list_orphaned_objects}` are added. Migrate by reading or discarding the returned outcome.
- **Breaking:** `ImageOwners::open` returns `Result<_, ImageOwnersError>` and `ImageOwners::owner` is replaced by `inserters`, which answers every editor who inserted a reference. Migrate by propagating the error and checking each inserter.
- **Breaking:** `DocumentsError` gains `SubtreeChanged` (409, refusal `folder_not_empty`) and `OwnershipChanged` (409, refusal `forbidden`). Migrate exhaustive matches by adding the variants.
- **Breaking:** `Allowance::Save` carries the document `owner` and `Allowance` is no longer `Copy`; `quota_exempt` is replaced by `charged_to` and `new_bytes_only`, and `SessionTerms` replaces `quota_exempt` with `quota_user` and `new_bytes_only`. Migrate by passing the document's owner to `Allowance::Save`.
- **Breaking:** recursively deleting a folder requires `delete` on every folder in the subtree; the caller's own documents are trashed with it, and every document another user owns is moved to its owner's root in the same transaction, with a path change on the Drive feed, instead of being trashed. Migrate callers that expected a collaborator's document to be trashed with the folder.
- **Breaking:** `EngineDoc::apply_update` returns an `UpdateImport` naming the peers an update added and whether part of it is pending; `Joined` gains `peer`; `CollabError` gains `PeerNotAssigned`, `MissingDependencies` and `UpdateContended`. Migrate engines by returning the import status and exhaustive matches by adding the variants.
- **Breaking:** `DocumentEngine::check_update` takes the caller's assigned peers (`Option<&[u64]>`) and refuses from the encoded header alone an oversized, non-incremental or inconsistent update, or one carrying operations of an unassigned peer. Migrate engines by validating the header against the bounds and `peers`.
- **Breaking:** `DocumentCollabRepository::append` takes an `UpdateAuthor` (user, actor, admitted level), as does the new `append_on`, `CollabUpdateRow` gains `actor`, and `DocumentEditSessionRepository::set_checkpoint` takes the sequence number the checkpoint covers. Migrate callers by passing the author and covered sequence number.
- **Breaking:** `DocumentCommentRepository::{insert, insert_granting, update_body, update_body_granting}` and `DocumentGrantRepository::insert` take a `&mut PgConnection` after `&self`, so the write and its notification event commit in one transaction. Migrate by opening a transaction on the write pool and passing `&mut *tx`.
- **Breaking:** `DocumentServiceParts` gains `notifications: DynNotificationSink`, and `CommentRequest` and `EditCommentRequest` gain `mention_options: Vec<MentionOption>`. Migrate struct literals by passing the runtime's sink (`systemprompt_runtime::notification_sink`, or `NotificationsUnavailableSink` without the `notifications` feature) and `mention_options: Vec::new()`.
- **Breaking:** `DocumentsError` gains `Notifications(NotificationSinkError)` and `NotificationKind(NotificationKindError)`. Migrate by adding arms (or a wildcard) to exhaustive matches.
- **Breaking:** `AgentCapability` and `AgentRefusal` gain a `BroadRewrite` variant, declared before the existing tail variants, so the implicit discriminants of `AgentCapability::{ReviewProposal, CopyOut, EditTemplate}` and `AgentRefusal::{SuggestOnly, RateLimited, OverrideLooser, MoveRefused}` move up by one. Migrate by adding arms (or a wildcard) to exhaustive matches, and by not persisting or comparing these enums as integers.
- **Breaking:** `DocumentRepository::list_visible` and its `VisibleListing` argument are removed; folder listings go through `list_visible_children`. `DocumentRepository::ids_in_folder` is removed.
- **Breaking:** `DocumentLockRepository::release_all(&DocumentId)` is replaced by `release_all_for(&[DocumentId])`, and `DocumentPresenceRepository::clear(&DocumentId)` by `clear_for(&[DocumentId])`. Migrate single-document callers by passing a one-element slice.
- **Breaking:** `DocumentFolderRepository::move_to` takes a `FolderRelocation` (folder, destination, the destination's ancestor chain read before authorizing, and the mover) and answers `FolderMoveOutcome::{Moved, Refused, DestinationChanged}` instead of a row count. Migrate callers by passing the relocation and matching the outcome.
- **Breaking:** `DocumentRepository::path_changes_since` takes the caller's `SubjectPairs`, and `DocumentPathChange` gains `access: Option<SubjectAccess>` (`Lost` or `Gained`) for a row recording one subject's change of access. Migrate by passing the reader's pairs and matching the new field.
- **Breaking:** moving a folder moves every document another user owns in the moved subtree to its owner's root in the same transaction, with a path change on the Drive feed, instead of carrying it under the new parent's grants. Migrate callers that expected a collaborator's document to move with the folder.
- **Breaking:** `FilesConfigYaml` gains `issues: IssuesConfig`, and **Runtime:** `DocumentsRuntimeParts` gains `issues: Arc<IssueService>`. Migrate struct literals by adding `issues: IssuesConfig::default()` to `FilesConfigYaml` and an `IssueService` built from `IssueReportRepository::new(&db)`, the notification sink and `files_config.issues_config().clone()`.

### Added

- Issue reports (`IssueReportRepository`, `IssueService`, `IssuesConfig` under `files.issues`, files migration `025`, the `issue_report_retention` job).
- Comment mentions and shares record `document.comment.mentioned` and `document.shared` notification events inside the write's transaction.
- `DocumentGrantRepository::insert_bounded`, migration `036_document_path_change_subject`, the `document_collab_reaper` and `document_agent_read_retention` cluster jobs.

### Fixed

- Live editing, the commit path, folder moves and deletes, sharing, the Drive change feed, uploads and Office packages are hardened; see the workspace CHANGELOG for each change.

## [0.75.0] - 2026-10-28

### Breaking

- `CreateRequest` gains `content: Option<RichText>`, `ImportRequest` gains `mime_type: Option<String>`, `NewGrant` gains `reason` and `via_agent`, `collab::Joined` gains `seq`, `DocumentRepositories` gains `uploads`, `grant_events` and `access_requests`, and `DocumentNodeIdRepository::insert` is an associated function taking a connection. Migrate struct literals and calls.
- `DocumentsError` gains `Agent(AgentRefusal)`, `Granted` and `TargetAccess` gain `agent_cap`, and `DocumentLimits` gains `agent_edits_per_hour`. Migrate by adding arms (or a wildcard) to exhaustive matches and the fields to struct literals.
- The Loro CRDT moves out of `systemprompt-files` into the new `systemprompt-engine-loro` crate behind the `documents::engine::DocumentEngine` seam (with `FormulaEngine`, `RenderEngine` and the engine-agnostic `ModelView`); `systemprompt-files` links no CRDT crate. `CollabService::new` takes a third argument, `Option<Arc<dyn DocumentEngine>>`; with `None` every session call answers `CollabError::EngineUnavailable` (HTTP 503, `collab_engine_unavailable`). `CollabError::{Crdt, Encode, Reconcile}` collapse into `Engine(EngineError)`, and `InvalidUpdate`/`InvalidVersion` carry an `EngineError`. `collab::model::{BODY_CONTAINER, SEED_PEER, seed, body_text, replica_peer}` move to `systemprompt_engine_loro`. Migrate by passing `Some(Arc::new(LoroEngine))` and enabling the facade's `files-collab-loro` feature (`full` implies it); `files-collab` alone mounts the routes without an engine.
- Core ships no third-party frontend code: the vendored `loro-crdt` WebAssembly (`assets/documents/wasm/*`), the `documents-collab.js` client and the `sp-document-editor` component are removed. Migrate by vendoring a Loro client in the installation that serves the editor, at `systemprompt_engine_loro::WIRE_VERSION`.
- `DocumentAccessService::new` takes a fourth argument, `Arc<dyn SubjectDirectory>`, which resolves grant subjects to users. Migrate by passing `UserSubjectDirectory` from `systemprompt-runtime` or your own implementation.
- `DocumentService::transfer_ownership` takes a `TransferRequest { to, to_display }`; a `to_display` of `None` is refused with `UnknownSubject`. Migrate by resolving the recipient with `active_user_display` and passing the result.
- `DocumentsError` gains `UnknownSubject`, `OwnNotGrantable`, `ShareBelowExisting` and `Directory`. Migrate by adding arms (or a wildcard) to exhaustive matches.
- `own` is not a grantable level: `share` refuses it with `invalid_params` (`DocumentsError::OwnNotGrantable`), and a stored `own` grant confers `share`. Migrate by transferring ownership instead.
- `collab::Left::checkpoint` is a `Checkpoint` (`Unchanged`, `Committed`, `Conflict { head, proposal }`) instead of `Option<DocumentRevisionId>`, and the leave route answers `{closed, checkpoint, checkpoint_revision, head_revision?, proposal_id?}`. Migrate by matching on `Checkpoint` or calling `Checkpoint::revision`.
- `DocumentCollabRepository::append` returns `Option<i64>` (`None` for a closed session) and `set_snapshot` takes a `SnapshotMark`. Migrate by handling `None` and passing the snapshot's hash, sequence number and version vector.
- `DocumentRepository::rename`, `move_to`, `trash` and `restore` take the document's path before and after the change (`PathMove`, or the single path a trash leaves or a restore returns to) and record it in `document_path_changes`. Migrate by passing the paths; `DocumentService` callers are unchanged.
- `CommitRequest` gains `op_id`, and `CommitOutcome` becomes `Committed(Box<LandedCommit>)`, `Merged(Box<LandedCommit>)` or `Conflict { head: Box<DocumentRevision>, base }`; `LandedCommit` carries the revision, the stored `WriteAck`, the new head's projection and `replayed`. Migrate by passing `op_id: None` (or the caller's op id) and matching `Committed(landed)` / `Merged(landed)`, reading `landed.revision` and `landed.ack.merged_from`.
- `DocumentService::find_write_ack` and `record_write_ack` are removed and `StoredWriteAck` holds `user_id`, `request_sha256` and a typed `WriteAck`. Migrate by passing the op id in `CommitRequest::op_id`; the commit claims and replays acknowledgements itself.
- `DocumentRevision` gains `landed_at` and `NewRevision` gains `id`. Migrate struct literals by passing `DocumentRevisionId::generate()`; only revisions with `landed_at` resolve in reads, history, `restore` and `insert_from_revision`.
- `DocumentRepository::set_head_if` is replaced by `begin` and `DocumentRepository::move_head(conn, HeadMove)`, `DocumentNodeIdRepository::insert` takes a connection and refuses a second write of a revision's ids, `max_counter` is replaced by `reserve`, `DocumentWriteAckRepository::insert` by `claim`, and `DocumentRevisionRepository` gains `insert_in` and `mark_landed`. Migrate by writing heads, ids and acknowledgements inside one transaction from `DocumentRepository::begin`.
- `DocumentsError::TooLarge` gains `what`, and `DocumentsError` gains `OpIdMismatch`, `GuardMissing`, `SelfReview`, `InvalidAwareness` and `LiveSession`. Migrate by adding the field to patterns that bind it and arms (or a wildcard) to exhaustive matches.
- `carry_forward` takes `&CarryHints` (retired and rewritten ids) instead of a retired id set. Migrate by passing `CarryHints { retired, ..CarryHints::default() }`.
- `DocumentsConfig` gains `limits: DocumentLimits`. Migrate struct literals by adding `limits: DocumentLimits::default()`.
- `LandedCommit::projection` is a `Projection` (a replay projects the acknowledged revision), `DocumentsError` gains `InvalidParams`, `DocumentFolder::deleted_at` is `trashed_at`, and a revision without node ids refuses `NotFound` on read instead of minting them. Migrate by dropping `Option` handling and running the `document_node_ids_backfill` job.
- `SubjectDirectory` gains `search_users` and `roles`, and `UserSubjectDirectory::new` takes the extension-declared role names. Migrate implementations by adding both methods and construct the directory with `Vec::new()` or the registry's roles.
- `DocumentService::access_of` takes a `history: bool`, and `DocumentAccess` is `TargetAccess<Document>` with `target`, `inherited: Vec<InheritedGrant>`, `history` and `requests`. Migrate by passing `false` and reading `access.target` and `inherited[..].grant`.
- `ShareRequest` gains `message`, `NewGrant` gains `trace_id`, and `DocumentGrantRepository::delete` and `delete_for_document` take a `&GrantActor`. Migrate struct literals by adding the fields and pass the acting user and trace.
- `share` refuses a subject that does not exist (an inactive or unknown user, a role outside the role catalogue, or a value a listing dimension does not list) with `UnknownSubject`. Migrate by sharing with existing subjects.
- `DocumentRepository::set_owner` is removed; ownership moves through `DocumentGrantEventRepository::transfer_owner`. Migrate by calling `DocumentService::transfer_ownership`.
- `DocumentsError` gains `AccessRequestOpen`, `AccessRequestNotNeeded` and `AccessRequestClosed`, and `DocumentsConfig` gains `access_requests`. Migrate by adding arms (or a wildcard) and `access_requests: false` to struct literals.

### Added

- Agent access caps on documents: `documents::agents` (`DocumentActor`, `act_as`, `AgentCap`, `AgentCapability`, the `AgentAccessPolicy` seam and `register_agent_access_policy!`), owner overrides on documents and folders that only tighten (migration 024), the `document_agent_reads` audit, and the `limits.agentEditsPerHour` ceiling.
- The agent access policy is async and built from the database pool; `DocumentContext` carries `created_by_agent` and a nearest-first folder chain; `DocumentService::visible_documents` batch-checks view access; `sp-document-browser` gains `registerRowExtension` and `registerFolderMenuItem`.
- Comments v2: ranges of engine cursors, mentions, author edits and soft deletion (`edit_comment`, `delete_comment`, migration `016_document_comment_ranges`), the `document.comment.{updated, deleted, mentioned}` events, and `export_with_comments` writing open comments as native `w:comment`s in `docx` (`codecs::docx::with_comments`).
- `documents::model::ops::apply_rich_ops`: the `set_marks`, `set_block_type`, `insert_table`, `table_insert_row`, `table_insert_col`, `table_delete` and `insert_image` edit ops, applied through the `RichText` model of `html`, `markdown`, `docx` and `odt` documents. `documents::model::{formats, html_body}` hold the per-format import and materialise and the `<body>` splice.
- `docx` hyperlinks are native `w:hyperlink`s with an external relationship; new `docx` and `odt` tables are written natively.
- `documents::model`: the `RichText` model, HTML and Markdown import and materialise, `validate` with `Limits` and `EditLevel`, `carry_ids`, `rich_text_schema`. Collab protocol v2 join fields, comment-mode joins, and server-side validation of every update on a fork (`CollabError::UpdateRejected`). `docx` and `odt` co-editing through `model::office` segments, with every untouched package part kept byte for byte.

- `POST /api/v1/documents/{id}/collab/awareness` publishes a participant's `AwarenessState` (`color`, `mode`: `viewing`, `editing`, `commenting`, `cursor`, up to 16 `selection` ranges, `viewport`, `session_id`) through `DocumentService::publish_awareness`; `editing` needs edit access, the state lives on the presence row for `presenceTtlSecs`, and a malformed state is refused as `invalid_params`. `DocumentService::live_open` and `live_replay` back the channel.
- The grant audit log: every grant change (`granted`, `changed`, `revoked`, `revoked_all`, `transferred`, `expired`) writes a `document_grant_events` row in the same statement, read by holders of `share` through `documents_access` `history: true` and `GET /{id}/access?history=true`, and reported by the `report_document_sharing` view.
- Subject search, `GET /api/v1/documents/subjects?kind=&q=&limit=`, over users, the role catalogue and dimension values a provider can list.
- Folder sharing over REST: `GET`, `POST`, `DELETE /api/v1/document-folders/{id}/grants` and `GET /api/v1/document-folders/{id}/access`; inherited grants carry `inherited_from { folder_id, name }`.
- A share may carry a `message` (at most 1 KiB), sent on `document.shared` and shown in the bridge notification.
- Access requests behind `files.documents.accessRequests` (off by default): `POST /{id}/access-requests`, `POST /{id}/access-requests/{rid}/grant|decline`, the `document.access_requested` event, open requests in access read-backs, and `can_request: true` on a signed-in caller's `not_found` for an existing document.
- The share dialog searches subjects as a keyboard-navigable typeahead (`sp-subject-picker`), shows who has access read-only below `share`, names the folder of inherited grants, greys expired grants, confirms transfer and revoke-all, takes a message, lists access requests and shows the access history.
- `DocumentGrant::is_expired`, and grant listings and access read-backs carry `expired: true` for a grant past its `valid_until`.
- `DocumentService::propose_body` proposes a whole body on a base revision.
- PDF export. `DocumentService::render_pdf` renders a revision to A4 PDF from its codec projection with the `documents::pdf` module (pure Rust, standard fonts, images omitted, deterministic for a revision), capped by the new `files.documents.limits.pdfPages` (`DocumentLimits::pdf_pages`, default 500, refused as `TooLarge` beyond it).
- `collab::Joined::seq` and the join route's `seq` give the sequence number a client resumes from.
- Migration `013_document_path_history` adds `document_path_changes`, the path history the drive change feed merges with revisions under one `(timestamp, id)` cursor.
- `files.documents.limits` (`maxOpsPerCall`, `maxOpTextBytes`, `maxMarkdownBytes`, `maxCommentBytes`, `maxCollabUpdateBytes`, `maxCursorBytes`) with bounds validation; a request over a limit is refused as `too_large` (HTTP 413).
- The `document_node_ids_backfill` job mints node ids for revisions written before ids existed, once per release under a release stamp.
- Migration `011_document_revision_landing` adds `document_node_counters`, `document_revisions.landed_at` (backfilled along each head chain) and `document_write_acks.user_id` and `request_sha256`.
- Migration `012_document_collab_session_seq` adds `document_edit_sessions.next_seq` and `snapshot_vv`, so live-edit sequence numbers are assigned per session under a row lock and a snapshot records the version vector it covers.
- Migration `015_document_grant_events_and_access_requests` adds `document_grant_events` and `document_access_requests`.
- `DocumentLimits::pdf_pages` is validated with the other limits at boot (`files.documents.limits.pdfPages`, 1 to 5,000).
### Changed
- A commit's revision, node ids, op id acknowledgement and head move commit in one transaction on the primary database, and the head's compare-and-swap also refuses a foreign lock.
- An `op_id` replays only for the same user and the same request, over MCP and HTTP alike; a different request under a used op id is refused as `op_id_mismatch`.
- `replace_text` and `delete` on a node id require `expected_hash`; a quote target resolves its own.
- An agent's `mode: land` in a folder whose agent edit policy is `propose` is refused, and a proposer cannot accept their own proposal (`self_review`).
- `insert_from_revision` copies Word paragraphs and tables and PowerPoint paragraphs with their formatting.
- The drive change feed (`GET /v1/bridge/drive/changes`) reports `deleted` when a document is trashed, `created` with the head revision when it is restored, and `renamed { from }` when it is renamed or moved, merged with revisions under one `(timestamp, id)` cursor.
### Fixed
- Deleting, trashing, restoring, moving, transferring ownership and revoking every grant on a document are the owner's alone.
- A share holder can no longer change a grant whose subject holds more than they do.
- A caller with no access to a document or folder is answered `not_found` without the owner's id, and a caller below `share` reads back only the owner and their own level.
- Transferring ownership requires an active recipient and notifies the previous owner and present users.
- Revoking a grant, revoking every grant and grant expiry clear the affected users' presence and live editing state, and document events reach only present users who still hold access.
- The presence sweep job deletes expired grants.
- `document.shared` is emitted once per recipient user, with role subjects resolved to their active members, and never to the sharer.
- Live-edit sequence numbers are assigned per session in commit order, so a replica catching up never skips an update and a snapshot prunes only updates it covers.
- A live update is logged before any replica applies it, so a refused or failed update leaves the cache unchanged.
- Leaving a session whose checkpoint conflicts with the head keeps the session open and keeps the live body as a proposal by the last editor; checkpoints run as the last editor and a leaving viewer never commits.
- Trashing a document commits its live body, closes its edit session and clears its locks and presence.
- The web editor advances its sent version only after the server accepts an update, retries transient failures, diffs at code-point boundaries and rejoins on a sequence gap; `postEdits` takes a caller-supplied `opId` so a retry is idempotent.
- The drive change feed skips the revisions of a trashed document instead of failing the page.
- Concurrent commits by one writer no longer mint the same node id, and a node id is never stored without its row.
- A commit that loses the head race no longer leaves a revision that history, `restore` or `since_revision` would accept.
- A block inserted where an unrelated block was deleted no longer inherits its id, and a moved block keeps its id.
- A stale Office upload that changes a paragraph's formatting is reported as a conflict instead of landing without the formatting.
- An accepted proposal records its landed revision in the landing transaction, and the presence sweep reopens an accept that never landed.
## [0.74.0] - 2026-10-27

### Breaking

- `FilesConfigYaml` gains `documents: DocumentsConfig`. YAML without the key is unchanged; migrate struct literals by adding `documents: DocumentsConfig::default()`.

### Added

- A shared document store: folders with materialised ancestors, content-addressed blobs over `FileStorage`, documents with a compare-and-swap head revision, revisions with request attribution and stable node ids, subject-keyed grants, locks, presence, edit sessions, comments, proposals, write acknowledgements and a text index, each behind a repository in `DocumentRepositories` (migrations `003`–`010`).
- `DocumentService` and `DocumentAccessService`: template-based creation and import, node-guarded edit commits that merge onto a moved head, three-way merges for text formats and diff-on-head for Office files, sharing with stored subject display names, ownership transfer, comments, proposals with a folder `agent_edit_policy`, the trash (`trashed_at`, stored in `documents.deleted_at`), copies and restores that take the next free name, scoped listings, locks, presence, search and the bridge Drive feed, with document events on the AgUi stream.
- `DocumentCodec` implementations for HTML, Markdown and plain text, and (feature `office`) `.docx`, `.xlsx`, `.pptx`, `.odt`, `.ods` and `.odp`, with the `CodecRegistry`; edit ops target nodes by id or by quoted text and accept Markdown content; every codec writes `pending` placeholder blocks.
- A document template registry merging `DocumentTemplateProvider` registrations with `services/documents/templates/*.yaml`, with core blank templates.
- `CollabService` (feature `collab`): live HTML and Markdown co-editing over a Loro CRDT with snapshotting and `edit_session` revisions.
- Jobs: blob GC, presence sweep, `document_trash_purge` and `document_collab_update_sweep`; the `report_document_revisions` view.
- The `files.documents` configuration (`enabled`, upload limit, GC grace, presence TTL, `trashRetentionDays`); the store is disabled by default.
- Web Components for the document store, shipped through `FilesExtension::required_assets` to `assets/documents/{js,css,wasm}` with `loro-crdt` 1.17.1 vendored as WebAssembly.

## [0.73.0] - 2026-10-25

### Breaking

- **Breaking:** `FilesConfig::{init, get, get_optional}` are removed; the context owns the value. Migrate by reading `AppContext::files_config()`, building one with `FilesConfig::from_profile` or `FilesConfig::new` at a composition root, and passing it to `FilesAiPersistenceProvider::new`.
- **Breaking:** `FilesConfig::ensure_storage_structure` returns `Result<(), FilesError>` (`FilesError::StorageDir` names the directory) and `FilesConfig::validate` no longer touches the filesystem; `FilesConfigValidator` lives in this crate.

### Changed

- The files owner reassignment is declared through `systemprompt_database::owner_reassignment!` and runs in one transaction.

## [0.70.0] - 2026-10-18

### Changed

- `file_ingestion` runs on every replica.

## [0.63.0] - 2026-10-07

### Breaking

- `FilesError::Storage` and `FileUploadError::Config` are removed (never produced). Migrate by dropping the match arms.
- Providers read host handles through the typed `Dependencies` map (`ctx.get::<T>()?`) instead of the removed `db_pool::<T>()`/`app_context::<T>()` accessors.

### Added

- `OwnerReassignment` over `files`, so an account merge moves file ownership in this crate's own transaction.

### Changed

- Identifiers are typed and errors keep their sources on the one `RepositoryError` model.

### Fixed

- File metadata is read through `tokio::fs` instead of blocking the async runtime, and a failed file-size read is logged rather than discarded.

## [0.59.0] - 2026-09-22

### Removed

- The prefix-duplicate index on `content_files` (migration `002_prune_prefix_duplicate_indexes`), a strict column prefix of a non-partial covering index; the `CREATE INDEX` line goes from the base schema in the same change.

## [0.53.0] - 2026-09-15

### Changed

- `FilesAiPersistenceProvider::{find_by_id, list_by_user}` return `AiProviderError::Internal` when a stored file's metadata cannot be serialised instead of substituting `null`.

## [0.50.0] - 2026-09-10

### Changed

- The files config validator passes the new bundle-root argument to `AppPaths::from_profile`, matching the loader's services-bundle composition.

## [0.44.0] - 2026-09-02

### Changed

- Uploads go through the `FileStorage` trait rather than writing to a local path directly, so a shared mount or an object-store backend needs no change here.

## [0.38.0] - 2026-08-25

### Changed

- A malformed stored `context_id` is warned about and dropped when reading file content rows, instead of being constructed unchecked.

## [0.27.0] - 2026-07-29

### Added

- `files.cacheControl` in `files.yaml`, exposed as `FilesConfig::cache_control`, sets the `Cache-Control` served for files under the configured `urlPrefix`. Unset by default, since stored files are not content-hashed, and validated at config load as a non-empty printable header value.

## [0.25.0] - 2026-07-27

### Changed

- File ingestion and the upload validator's extension lookup resolve through `systemprompt_models::mime` rather than two local tables.

### Fixed

- The upload validator strips parameters from the submitted MIME type before matching. A browser sends `Content-Type: text/plain; charset=utf-8` on a multipart part, which no allowlist entry matched, so a plain text file was rejected as a disallowed type; the same parameter also carried a script type past the blocklist, where only the closed allowlist that follows stopped it.

## [0.23.0] - 2026-07-24

### Removed

- The unused `ai_image_stats` view (migration `001_drop_ai_image_stats_view`).

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- `File.metadata` is `sqlx::types::Json<FileMetadata>` (was `serde_json::Value`) and the fallible `File::metadata()` accessor is removed — use the typed field directly. `FileMetadata` gains a flattened `extra` map so historical metadata shapes decode losslessly.
- `ContentFile.role` is `FileRole` (was `String`) and `parsed_role()` is removed. `FileRole` serde casing is now `snake_case`, so the OG-image role serializes as `"og_image"`.
- `InsertFileRequest.metadata` is `FileMetadata`. Fields sourced from `systemprompt_traits::{InsertAiFileParams, AiGeneratedFile}` remain `serde_json::Value` at the trait boundary.

## [0.16.0] - 2026-06-22

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. File storage surface unchanged.

## [0.9.2] - 2026-05-14

### Changed
- Normalize `CHANGELOG.md` format to align with workspace-wide maintainer style.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade crate to the Rust 2024 edition.

## [0.1.0] - 2026-02-02

### Changed
- Promote crate to a stable `0.1.0` release aligned with the workspace baseline.

## [0.0.13] - 2026-01-27

### Changed
- Bump version for workspace consistency.

## [0.0.3] - 2026-01-22

### Added
- Add migration system infrastructure for schema rollout.

### Fixed
- Fix schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Added
- Distribute schema registration so each domain crate owns its SQL via the `Extension` trait.

### Changed
- Remove the centralised module loader path from `systemprompt-loader` in favour of per-crate ownership.

### Fixed
- Correct `include_str!` paths that previously pointed outside the crate directory.
- Ensure the crate compiles standalone when consumed from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release of `systemprompt-files`.
