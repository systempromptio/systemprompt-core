# Shared Document Store

How systemprompt-core stores, versions, permissions and syncs documents: Word, Excel, PowerPoint, OpenDocument, HTML, Markdown and plain text, edited by people and by agents.

The document store lives in the `files` domain (`crates/domain/files/src/documents`). It adds folders, content-addressed revisions, grants, locks and presence on top of the existing `FileStorage` backend. Postgres holds metadata only. The bytes of every revision live in the configured storage backend (local, GCS or S3).

## Model

- **Folders** form a tree. Each folder stores its materialised ancestors, so a grant on a folder covers its whole subtree. Folder names are unique within a parent.
- **Documents** belong to a folder, carry one format and point at a head revision. Names always carry the format extension.
- **Revisions** form a chain. Each revision records its parent, the SHA-256 of its bytes, an author, an actor kind (`user`, `agent`, `bridge`, `import`, `system`), the origin that produced it, for structured edits the edit operations, and the request it was written in: session, conversation context, trace, task and, for an agent, the user it acted for. The `report_document_revisions` view exposes these columns with each revision's size, hiding a deleted author's rows.
- **Blobs** are stored once per SHA-256 under `documents/blobs/<sha[0..2]>/<sha>`. Identical content is shared across revisions. The static `/files/*` server only serves `files/`-prefixed ids, so blobs are reachable only through the authorised document routes.

The head of a document moves with a compare-and-swap on the previous head, so concurrent writers never overwrite each other silently.

## Formats and the codec seam

| Format | Extension | Codec feature |
|---|---|---|
| Word | `.docx` | `office` |
| Excel | `.xlsx` | `office` |
| PowerPoint | `.pptx` | `office` |
| OpenDocument text, spreadsheet, presentation | `.odt`, `.ods`, `.odp` | `office` |
| HTML | `.html` | always |
| Markdown | `.md` | always |
| Plain text | `.txt` | always |

Every format is handled through one trait, `DocumentCodec` (`systemprompt-provider-contracts`). A codec projects bytes into a tree of nodes, applies edit operations, renders an HTML preview, extracts plain text for search and instantiates templates. Extensions register their own codec with `register_document_codec!`. For each format the registry picks the codec with the highest `priority()`, so an extension can replace a core codec.

Office codecs edit by rewriting only the parts they touch inside the package (for example `word/document.xml`) and copy every other entry byte for byte. Styles, formulas and unknown elements survive an edit.

The `office` feature of `systemprompt-files` is exposed by the facade as `files-office` (included in `full`). Without it the HTML, Markdown and text codecs remain.

### Projections and anchors

A codec projects a document into nodes with a kind, text, an 8-hex-character hash of the normalised text and a positional path, for example `body/p[12]` in Word, `sheet[Sales]/B7` in Excel, `slide[3]/shape[2]/p[0]` in PowerPoint and `line[N]` in Markdown and text. A path names a position, so it shifts when a block is inserted above it.

The service gives every node an opaque, stable id instead: `<writer>.<counter>`, where the writer is a short tag of the session or execution that committed the revision and the counter rises per writer within the document. Ids are stored per revision. A new revision inherits its parent's ids by aligning blocks of the same kind and hash, keeping ids for changed blocks between aligned ones, retiring the ids an edit deleted and minting ids for inserted blocks. An id therefore names the same block in every revision until the block is deleted, and the tool surface, `document://<document>#<node>` addresses and acknowledgements use ids only. The positional path stays available as a debug field.

A line in the hashline read format pairs the id with its hash: `k3x9q2.14:a3f1c2d0| text`, or `Sales!B7:9c2e1f0a| 1234.5` for a cell.

Edit operations are `ReplaceText`, `InsertAfter`, `Delete`, `SetCells`, `InsertSlide`, `StrReplace`, `SetHtml`, `SetText`, `AppendMarkdown`, `Restore` and `InsertFromRevision`. Operations that target a node carry the hash the writer saw (`expected_hash`), or quote the block's text.

## Templates

New documents are created from a template. Two seams supply them:

- `DocumentTemplateProvider`, registered with `register_document_template_provider!`, for templates compiled into an extension.
- YAML files under `<services>/documents/templates/*.yaml`, each declaring `id`, `format`, `name`, `description`, `file`, an optional `priority` and `placeholders` (`key`, `label`, optional `default`).

The registry merges both sources and the highest priority wins per template id. Two catalog files that declare the same id are rejected. Core ships blank templates for text, Markdown and HTML, and with `office` for docx, xlsx, pptx, odt, ods and odp. They are generated by the codecs, so no binary fixtures are involved.

## Access model

Access is checked in two steps and both must pass.

1. **Grants.** A grant names a subject (`subject_kind`, `subject_value`), a level and an optional expiry. Levels are ordered `view < comment < edit < share < own`. The subject kinds are `user`, role names and every dimension registered through the subject-attribute provider seam, such as group or project in an extension. A subject holds the highest unexpired level granted on the document, its folder or any ancestor folder. The owner holds `own`. Creating a folder grants its creator `own`.
2. **Policy.** The authz hook must allow a `document.access` request whose entity is the nearest folder. Folders are registered as authz entities when created, and a boot reconcile registers any that are missing, so an operator governs folders in `rules.yaml` like any other entity. Documents at the root are checked against a root folder entity.

Administrators are not special-cased. They reach documents through grants to their role. Everything fails closed: an unknown target, a subject provider error or a hook denial is a refusal, never a partial answer. Hook decisions appear in the governance audit trail. Listings filter in SQL with the same grant predicate. Anonymous share links are not part of this release.

Required levels: view and read need `view`, edit and commit need `edit`, listing or changing grants needs `share`, deleting and moving need `own`.

## Revision engine

`DocumentService` is the single write path for MCP, HTTP and the bridge. A commit carries an optional base revision and either whole bytes or edit operations.

- **Edit operations apply to the head, in every format.** Each anchored operation names a node by id and quotes its hash. A commit is refused only when a node it touches changed at the head; the refusal carries that node's current text. An edit elsewhere in the document does not conflict, and a commit that landed while the head moved past its base is marked as merged. The base only says which revision the ids and hashes were read from.
- **Whole-document operations.** `set_text`, `set_html` and `restore` touch every node, so they apply only when their base is the head.
- **Bytes on the head.** Whole bytes whose base is the head are stored as given.
- **Bytes on a stale base, text formats.** HTML, Markdown and text are merged three-way against the common base. An overlapping change is reported as a conflict.
- **Bytes on a stale base, Office formats.** The upload is diffed against its base: a changed heading, paragraph or list item becomes a text replacement, a removed one a delete and a new one an insert after its previous sibling. The diff is checked to rebuild the upload from the base, then applied to the head under the same node guards and marked as merged. A change the diff cannot express, such as an added table, a removed slide or a style change, is reported as a conflict that names the head and the base.
- **History.** `restore` makes an earlier revision the head as a new revision, and `insert_from_revision` copies a block's text from an earlier revision, so text is never retyped.
- **Idempotent writes.** A write may carry an `op_id`. Its acknowledgement is stored for seven days, and a repeat with the same `op_id` on the same document returns the stored acknowledgement marked as replayed instead of applying again.
- **Locks.** A lease held by another holder (bridge, editor or agent) refuses the commit.
- **Size.** Uploads are bounded by `maxUploadBytes`.

Each commit updates the per-document search index (full-text over the extracted plain text). Revisions, presence and locks emit `document.revision.created`, `document.presence`, `document.lock` and `document.conflict` events on the AgUi stream (`/api/v1/stream/agui`), delivered to the owner and to users with presence on the document. Comments emit `document.comment.created` and `document.comment.resolved` to the same users; a grant emits `document.shared` to them and to the user it names, and an ownership transfer emits `document.ownership_transferred` to the new owner. Clients filter on the `document.` prefix.

## MCP tools

Document tools are exposed through MCP and authorise as the calling user, never as the server. There are fifteen tools. Read-only tools are annotated as such.

| Tool | Purpose |
|---|---|
| `documents_list` | Documents by scope: one folder with its subfolders, documents shared with the caller, the caller's own, or the ones the caller last edited, commented on or opened. Filtered by name, sorted by name, last update or last touch, paged by cursor. Each item carries its size and the caller's access level. |
| `documents_search` | Full-text search over the head revision of every visible document. |
| `documents_create` | Create from a template, optionally applying edit operations in the same first revision. |
| `documents_read` | A revision as hashlines, by outline, text, node range, one node, spreadsheet cells or search; the changes since a revision; or the document's metadata and who is in it now. |
| `documents_edit` | Apply edit operations as one revision on the head, guarded per node, optionally idempotent by `op_id`. `answering` records the revision on a comment; an edit that removes text a comment is anchored to is refused unless `allow_detach` is set. |
| `documents_comment` | Create, list and resolve comment threads anchored to a node and, optionally, a quote inside it. |
| `documents_history` | Revisions, newest first, with author, actor kind and message, and with `verbose` the session, context, trace, task and on-behalf-of user. |
| `documents_share` | Grant a subject access to a document or a folder subtree, transfer a document's ownership, or revoke every grant on it. |
| `documents_access` | The owner, the grants on a document with each subject's display name, the grants it inherits from its folders, and the caller's level. |
| `documents_copy` | A new document owned by the caller from another document's head, with its own history. |
| `documents_update` | Rename a document or move it to another folder. |
| `documents_trash` | Move a document to the trash. |
| `documents_restore_document` | Restore a trashed document. |
| `documents_export` | A signed ten-minute download link, or the sanitised HTML preview for the document-preview app. |
| `documents_guide` | Working detail by topic: `editing`, `formats`, `sheets`, `slides`, `sharing`, `comments`, `lifecycle` and `refusal.<code>`. |

The server `instructions` carry the working contract: read the outline, edit by node id and hash or by quote with an `op_id`, anchor the next edit on the acknowledgement and resume later with the changes since your last revision, let the head edit win on a conflict and never resend the old text, and do not re-read between your own writes.

**Reading.** A line is `<node>:<hash>| <text>`; a cell is `<sheet>!<cell>:<hash>| <value>`. The text shows structure as the outline does: `# ` by heading level, `- ` for a list item, `| ` for a table cell and `## slide N` at each slide. The outline folds a run of list items after three as `gap blocks=N`. `view: node` reads one node and its children, `view: search` the blocks containing a text, `view: meta` the name, place, access, head, size and who is present, and `view: changes` with `since_revision` the blocks changed since that revision, a `gap blocks=N` line for each run of unchanged blocks and a `gone <id>` line for each deleted block.

**Editing.** Operations are `replace_text`, `insert_after`, `delete`, `str_replace`, `set_cells`, `insert_slide`, `set_text`, `set_html`, `append_markdown`, `restore` and `insert_from_revision`. Every target in a call resolves against the document as it stood before the call, operations apply in order, and all apply or none. Each codec reports the node kinds `insert_after` accepts for its format. Every write takes a revision message, which history shows.

**Placeholders.** A `pending` block stands for content still to be written and carries its intent as text, so a document can be laid out as a skeleton and filled section by section. `insert_after` takes kind `pending`, and in Markdown content a line `<!-- pending: intent -->` is a pending block. Each format stores it natively: a highlighted run reading `[pending: intent]` in Word, a text paragraph in PowerPoint and OpenDocument, a cell in a spreadsheet, a `<p data-pending="intent">` in HTML, the comment followed by the intent in italics in Markdown, and a `[pending: intent]` line in text. A read shows it as kind `pending`; `replace_text` on it writes ordinary content in its place.

**Comments.** A comment is anchored to a stable node id and, optionally, to the nth occurrence of a quote inside that node; a reply carries its parent's anchor. Edits elsewhere never move an anchor. An edit that deletes the node or the quoted words is refused with `anchors_affected`, listing the comments, unless it sets `allow_detach`; a detached comment keeps its node and drops its quote. Bridge uploads and whole-file writes detach without refusing. Commenting needs `comment` access.

**Lifecycle.** A trashed document leaves listings, and reads, edits and shares refuse it as `not_found` with `trashed: true`; only a caller who could reach it learns it is trashed. Trashing and restoring need `own`. A document restored into a deleted folder lands at the root, and a name taken in the meantime gains a ` (restored)` suffix. Trashed documents are deleted, with their revisions, after the retention period. Transferring ownership needs `own`, and the previous owner keeps `edit`.

**Results.** A result is a one-line summary plus structured content, sent once. Outputs are terse by default; `verbose: true` on a read, edit, history or list adds `document://<document_id>@<revision_id>` URIs, content hashes, each node id's positional path and the revision attribution. A write acknowledges the lines it changed or created, so the next edit anchors on them without a read; a write landed on a moved head says `merged`, and a replayed `op_id` says `replayed`.

**Refusals.** A refused call is a tool result with `isError: true`. Its structured content carries a stable `code` (`stale_base`, `guard_mismatch`, `find_none`, `unsupported_op`, `unsupported_format`, `template_failed`, `forbidden`, `not_found`, `locked`, `anchors_affected`, `invalid_params`) and the detail the failure knows: the head revision and who wrote it, a node's current text and hash, a match count, the accepted node kinds, the owner to ask for access, the comments an edit would detach, or that a document is in the trash. The text names what to do next.

**Export.** `native` as a URL works for every format, and `html` as a preview works for every format. `md` or `html` as a URL is offered only for a document already in that format, and `pdf` is refused as `unsupported_format`.

The tool handlers are composed by an extension's MCP server binary; core provides the handlers and the service behind them.

## HTTP API

Routes mount under `/api/v1/documents` and `/api/v1/document-folders` with user authentication.

| Route | Purpose |
|---|---|
| `GET`, `POST /api/v1/documents` | List by `scope` (`folder`, `shared`, `mine`, `recent`), `folder`, `query` and `sort` (`name`, `updated`, `recent`), with each document's `size_bytes` and `my_access`; create from a template, or import by multipart upload. |
| `GET`, `HEAD`, `PATCH`, `DELETE /api/v1/documents/{id}` | Metadata, rename and move, and trash. The `ETag` is the head revision. |
| `POST /api/v1/documents/{id}/restore`, `/copy` | Restore from the trash; copy into a new document. |
| `GET /api/v1/documents/{id}/read` | The `documents_read` views: `view=outline`, `text`, `changes` (`since_revision`), `node` (`node_id`), `search` (`text`) or `meta`, at `revision_id` or the head. |
| `GET`, `POST /api/v1/documents/{id}/comments`, `POST /api/v1/documents/{id}/comments/{comment_id}/resolve` | Comment threads. |
| `GET /api/v1/documents/{id}/access` | Owner, grants with display names, inherited grants and the caller's level. |
| `POST /api/v1/documents/{id}/transfer`, `DELETE /api/v1/documents/{id}/grants/all` | Ownership transfer; revoke every grant. |
| `GET /api/v1/documents/subject-kinds` | The grant subject kinds this installation accepts. |
| `GET`, `PUT /api/v1/documents/{id}/content` | Download with range and conditional support. Replace the bytes with `If-Match: "<revision_id>"`; a stale base merges as described above or returns `409` with the head and base. `?op_id=` makes the write idempotent. |
| `POST /api/v1/documents/{id}/edits` | Structured edit operations, with an optional base revision, `op_id`, `answering` and `allow_detach`. |
| `GET /api/v1/documents/{id}/revisions` | Revision history. |
| `GET /api/v1/documents/{id}/projection`, `/preview` | Node projection and HTML preview. |
| `GET`, `POST`, `DELETE /api/v1/documents/{id}/grants` | Grants. |
| `PUT`, `GET /api/v1/documents/{id}/presence` | Presence beacons. |
| `POST`, `DELETE /api/v1/documents/{id}/lock` | Lease lock. |
| `GET`, `POST`, `PATCH /api/v1/document-folders` | Folders: list, create, rename, move. |

With the `files-collab` facade feature (part of `full`), four further routes co-edit HTML and Markdown documents live. See [Live co-editing](#live-co-editing).

Errors map to `409` conflict and stale anchor, `403` forbidden, `404` not found, `413` too large, `422` integrity mismatch and `423` locked. See [reference/http-api.md](reference/http-api.md) for the common error shape.

A write records the request's session, trace and task, and the conversation named by an `x-context-id` header. A merged write answers `merged_from`, and a replayed `op_id` answers the stored response with `replayed: true`. Errors map to `409` conflict and stale anchor, `403` forbidden, `404` not found, `413` too large, `422` integrity mismatch and `423` locked. See [reference/http-api.md](reference/http-api.md) for the common error shape.

## Live co-editing

HTML and Markdown documents can be edited by several people at once. The document body is a [Loro](https://loro.dev) CRDT: one text container named `body`, holding the inner markup of `<body>` for HTML (the whole document when it has no `<body>`) and the full text for Markdown. Office formats are not co-edited and answer `400`.

| Route | Body | Response |
|---|---|---|
| `POST /api/v1/documents/{id}/collab/join` | `{"mode": "edit" \| "view"}` | `{session_id, snapshot_b64, vv, head_revision}` |
| `POST /api/v1/documents/{id}/collab/updates` | `{session_id, vv?, update_b64}` | `{seq, vv, missing_b64?}` |
| `POST /api/v1/documents/{id}/collab/leave` | `{session_id}` | `{closed, checkpoint_revision}` |
| `POST /api/v1/documents/{id}/collab/cursor` | `{session_id, cursor}` | `204` |

Binary values are standard base64: `snapshot_b64` is a Loro snapshot, `update_b64` a Loro update, `vv` a Loro version vector. `edit` needs edit access and `view` view access. Updates are accepted only from a user who joined in edit mode (`403` otherwise). An update for a session that is no longer open answers `409`, and the client joins again. When the posted `vv` lacks changes the server holds, `missing_b64` carries them.

A document has at most one open edit session. Every accepted update is logged with a sequence number and sent as a `document.collab.update` event (`document_id`, `edit_session_id`, `seq`, `update_b64`, `author_id`) on the AG-UI stream of each user present on the document; there is no separate stream. Joins and cursor moves send `document.presence`, with `cursor` when one was posted.

The server stores a snapshot 30 seconds after the first unsaved update or after 200 updates, and commits the body as a revision with origin `edit_session` after 2 minutes of activity and when the last editor leaves, which also closes the session. For HTML the revision keeps the document around `<body>` from the revision the checkpoint is based on. A checkpoint that the revision engine merges with writes made outside the session puts the merged body back into the live text.

## Web components

The files extension ships native ES module Web Components for browsing, previewing and editing documents. The generator's asset job copies them into the web distribution:

| Path | Content |
|---|---|
| `assets/documents/js/documents.js` | Entry module; importing it defines every component |
| `assets/documents/js/*.js` | One module per component, plus the HTTP client, the event stream and helpers |
| `assets/documents/css/documents.css`, `documents-panels.css` | Styles built on the install's `--sp-` design tokens |
| `assets/documents/wasm/loro.wasm`, `loro.js` | Vendored `loro-crdt` 1.17.1 (MIT), the version of the server's `loro` crate |

| Element | Attributes | Uses |
|---|---|---|
| `sp-document-browser` | `scope` (`mine`, `shared`, `recent`) | `GET /documents` (folder navigation, scope and name filter); dispatches `sp-document-open` with `{id, format, name}` |
| `sp-document-preview` | `document-id`, `revision` | `GET /{id}/preview` in a sandboxed frame without scripts, `PUT /{id}/presence` |
| `sp-presence-bar` | `document-id` | `GET /{id}/presence`, `document.presence` events |
| `sp-document-editor` | `document-id`, `readonly` | the collab routes above, `document.collab.update`, `document.presence` and `document.revision.created` (`origin_kind = edit_session` is shown as a checkpoint) |
| `sp-sheet-grid` | `document-id` | `GET /{id}/projection`, `POST /{id}/edits` with `set_cells` |
| `sp-slide-text` | `document-id` | `GET /{id}/projection`, `POST /{id}/edits` with `replace_text` by node id and hash |
| `sp-share-dialog` | `document-id` | `GET /{id}/access` and `GET /subject-kinds`, `POST` and `DELETE /{id}/grants`, `DELETE /{id}/grants/all` and `POST /{id}/transfer` |
| `sp-history` | `document-id` | `GET /{id}/revisions`, a `restore` op on `POST /{id}/edits`, and `GET /{id}/read?view=changes` since a revision |
| `sp-comments` | `document-id`, `anchor-node` | `/{id}/comments`; hidden when the install answers `404` |
| `sp-proposal-list` | `document-id` | `/{id}/proposals`; hidden when the install answers `404` |

Every edit carries a fresh `op_id`, so a retried request replays its first acknowledgement. A refusal is shown with its error key. All components on a page share one AG-UI stream (`/api/v1/stream/agui`), because each user may hold at most ten streams.

The editor compiles WebAssembly. Core sends no `Content-Security-Policy`; an install that sets one must allow `'wasm-unsafe-eval'` in `script-src`.

The asset sources resolve inside the `systemprompt-files` crate, so they are declared optional: a build that compiles core copies them, and a runtime without the crate's sources keeps the copies already in its web distribution.

## Bridge Drive sync

The bridge keeps `~/<workspace>/Drive` in step with the store. It talks to five endpoints under `/v1/bridge/drive/`, authenticated like the bridge heartbeat.

| Endpoint | Purpose |
|---|---|
| `GET manifest` | Every document the user can view, with head revision and hash. |
| `GET changes?since=` | Revisions created since a keyset cursor. |
| `GET blob/{sha256}` | Bytes by hash. The bridge verifies the hash before it writes. |
| `POST commit` | Multipart metadata and bytes with `If-Match` set to the base revision. Returns the new revision, the merged revision and its hash when the upload landed on a newer head, or a conflict naming the head. |
| `PUT presence` | Editing beacons. |

Behaviour:

- A watcher debounces local writes by 1.5 seconds, ignores dot files, `~$` lock files, backup `~` files and `.tmp` or `.crdownload` partials, and commits the file as a revision.
- A pull loop polls the change feed every 20 seconds, or sooner when a `document.revision.created` event arrives on the bridge stream. Remote bytes are hash-verified and written atomically.
- A merged upload is recorded against the uploaded bytes, and the next pull writes the merged content.
- A conflicting local edit is renamed to `<name> (conflict from <host>).<ext>` and the head is pulled, so neither version is lost.
- A document deleted remotely moves into `Drive/.trash/<timestamp>/`. A local deletion is not propagated.
- Presence is reported from file activity and from agent tool calls that touch a path inside the Drive folder.
- Status shows in the tray, the GUI and `bridge drive status|sync|conflicts`. The `[drive] enabled` setting turns the loop off.

## Configuration

### Enabling

The store is off by default. While it is off, every `/api/v1/documents`, `/api/v1/document-folders` and `/v1/bridge/drive/` path answers `404`, the folder reconcile at boot and both background jobs do nothing, and an MCP server that composes the document tools refuses to start. Turn it on in `services/config/files.yaml`:

```yaml
files:
  documents:
    enabled: true
```

Two further switches follow from it. An installation's MCP server declaration (`services/mcp/<name>.yaml` `enabled:`) decides whether the document tools are served to agents. The bridge `[drive] enabled` setting decides whether a device syncs its Drive folder. A bridge that meets a server with the store off shows Drive as disabled, with no error, and checks again on its normal cadence.

### Keys

The store reads the `files.documents` section of `<services>/config/files.yaml`.

| Key | Default | Bounds | Meaning |
|---|---|---|---|
| `enabled` | `false` | | Mounts the routes and runs the jobs. |
| `maxUploadBytes` | 64 MiB | 1 MiB to 2 GiB | Largest accepted document body. |
| `blobGcGraceHours` | 24 | at least 1 | Age before an unreferenced blob can be deleted. |
| `presenceTtlSecs` | 90 | at least 1 | How long a presence beacon stays valid. |
| `trashRetentionDays` | 30 | at least 1 | How long a trashed document is kept before it is deleted. |

Config validation rejects values outside the bounds.

## Background jobs

| Job | Schedule | Behaviour |
|---|---|---|
| `document_blob_gc` | daily | Deletes blobs that no revision or edit-session snapshot references and that are older than the grace period, in batches of 500. |
| Presence sweep | every 60 seconds | Removes expired presence rows and write acknowledgements older than seven days. |
| `document_collab_update_sweep` | every 5 minutes | Deletes the update log of closed edit sessions. |
| `document_trash_purge` | daily | Deletes documents trashed longer than the retention period, with their revisions, node ids, grants and comments; the blob GC then collects their blobs. |

All run once per cluster, and return without work while the store is disabled.

## Planned

These items are designed and not part of the current release: the Web Components for browsing and editing, including the browser editor for live co-editing. Browser access today is the HTTP API.
