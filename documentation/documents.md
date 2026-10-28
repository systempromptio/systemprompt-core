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

1. **Grants.** A grant names a subject (`subject_kind`, `subject_value`), a level and an optional expiry. Levels are ordered `view < comment < edit < share < own`. The subject kinds are `user`, role names and every dimension registered through the subject-attribute provider seam, such as group or project in an extension. A subject holds the highest unexpired level granted on the document, its folder or any ancestor folder, up to `share`. Only the owner holds `own`: it is never granted, and actions that need it belong to the owner alone. Creating a folder grants its creator `share` over the subtree.
2. **Policy.** The authz hook must allow a `document.access` request whose entity is the nearest folder. Folders are registered as authz entities when created, and a boot reconcile registers any that are missing, so an operator governs folders in `rules.yaml` like any other entity. Documents at the root are checked against a root folder entity.

Administrators are not special-cased. They reach documents through grants to their role. Everything fails closed: an unknown target, a subject provider error or a hook denial is a refusal, never a partial answer. Hook decisions appear in the governance audit trail. Listings filter in SQL with the same grant predicate. Anonymous share links are not part of this release.

Required levels: view and read need `view`, edit and commit need `edit`, listing or changing grants needs `share`. Deleting, trashing, restoring, moving, transferring ownership and revoking every grant are the owner's.

A caller who holds no level on a document or folder is answered `not_found`, never `forbidden`, and is not told the owner. A refusal names the owner only to a caller who holds some level. Reading access back (`documents_access`, `GET /{id}/access`) shows the owner and the caller's level to every member; the grants and inherited grants are shown only to holders of `share`.

### Sharing

- `share` refuses level `own`, and refuses to change a grant whose subject already holds more than the caller (`forbidden` with `existing_level`).
- Transferring ownership names an active user; any other recipient is refused with `unknown_subject`. The previous owner keeps `edit`.
- Revoking a grant, revoking every grant and a grant's expiry remove the presence rows of the users it named, which also ends their live editing state. Role subjects are resolved to their active members; dimension subjects have no reverse index and rely on the recipient filter below.
- A grant past its `valid_until` grants nothing. Until the presence sweep deletes it, listings and access read-backs mark it `expired: true`.
- The subject must exist: an active user, a role in the role catalogue (the built-in `admin` and `user`, the roles extensions declare, and every role an active user holds), or a dimension value its provider lists. A dimension whose provider cannot list its values accepts any value. Anything else is refused with `unknown_subject`.
- `GET /api/v1/documents/subjects?kind=&q=&limit=` searches subjects for a share dialog: users by display name, name or email prefix, roles from the catalogue, and dimension values from providers that can list them, up to 20. A kind that cannot be listed answers `searchable: false`.
- A share may carry a `message` of at most 1 KiB, sent on `document.shared` and shown in the bridge notification.
- Folders are shared the same way as documents, over MCP (`documents_share` with `folder_id`) and REST (`/api/v1/document-folders/{id}/grants`). An access read-back lists each inherited grant with `inherited_from: {folder_id, name}`.

### Grant audit log

Every grant change writes a `document_grant_events` row in the same statement: `granted`, `changed` (with the old and new level), `revoked`, `revoked_all`, `transferred` and `expired` (from the presence sweep, with no actor). Each row records the target, the subject, the levels, `valid_until`, the acting user and the request's trace id. Holders of `share` read the newest 100 rows with `documents_access` `history: true` or `GET /{id}/access?history=true` on a document or a folder; callers below `share` never see them. The `report_document_sharing` view exposes the log for reporting and hides rows whose acting user has since been deleted.

### Access requests

With `files.documents.accessRequests` on, a signed-in user who holds no level on a document can ask its owner for one: `POST /api/v1/documents/{id}/access-requests {level, message?}`. A user holds at most one open request per document; a second is refused with `access_request_open`. The owner receives `document.access_requested`. Holders of `share` see open requests in `documents_access` and `GET /{id}/access` (and in the share dialog), and answer with `POST /{id}/access-requests/{rid}/grant` or `/decline`. Granting shares the requested level with the requester under every sharing rule above and writes the audit log; a request already answered is refused with `access_request_closed`.

A caller with no level is still answered `not_found`. With access requests on, a signed-in caller's refusal on a document that exists also carries `can_request: true`, which tells that caller the document exists. That is the trade-off of the setting, and why it is off by default: with it off, a document you cannot open is indistinguishable from one that does not exist.

## Revision engine

`DocumentService` is the single write path for MCP, HTTP and the bridge. A commit carries an optional base revision and either whole bytes or edit operations.

- **Edit operations apply to the head, in every format.** Each anchored operation names a node by id and quotes its hash (`expected_hash` is required for an id target; a quote target resolves its own). A commit is refused only when a node it touches changed at the head; the refusal carries that node's current text. An edit elsewhere in the document does not conflict, and a commit that landed while the head moved past its base is marked as merged. The base only says which revision the ids and hashes were read from.
- **Whole-document operations.** `set_text`, `set_html` and `restore` touch every node, so they apply only when their base is the head.
- **Bytes on the head.** Whole bytes whose base is the head are stored as given.
- **Bytes on a stale base, text formats.** HTML, Markdown and text are merged three-way against the common base. An overlapping change is reported as a conflict.
- **Bytes on a stale base, Office formats.** The upload is diffed against its base: a changed heading, paragraph or list item becomes a text replacement, a removed one a delete and a new one an insert after its previous sibling. The diff is checked to rebuild the upload from the base, then applied to the head under the same node guards and marked as merged. The rebuilt blocks must match the upload in formatting as well as text, compared by the codec's per-block style hash (Word paragraph and run properties, PowerPoint run properties, OpenDocument style names), so a stale upload that changes formatting is never landed without it. A change the diff cannot express, such as an added table, a removed slide or a formatting change, is reported as a conflict that names the head and the base.
- **History.** `restore` makes an earlier revision the head as a new revision, and `insert_from_revision` copies a block from an earlier revision, so text is never retyped. Word copies paragraphs with their runs and whole tables, and PowerPoint copies text paragraphs, in their native markup; other formats copy a block's text. A block a codec cannot copy intact, such as one that refers to an image or a link, is refused as `unsupported_op`.
- **Landing.** A commit lands in one transaction on the primary database: the revision, its node ids, the op id acknowledgement and the head's compare-and-swap commit together or not at all, so a commit that loses a race leaves no revision behind. The swap also refuses when another holder has the lock. History, the changes feed, `restore`, `insert_from_revision` and reads at a revision accept only landed revisions; a proposed revision is reached through its proposal.
- **Node ids.** Ids are `<writer>.<n>`, minted from a per-writer counter on the primary, so concurrent commits never mint the same id. A block an operation rewrote keeps its id; a block that moved keeps its id; a deleted block's id is never reused. In a stale Office upload a paragraph changed in place keeps its id, and elsewhere a block keeps an id only when its text is similar to the old block's. The changes view compares content, so a block that only moved is not reported as changed.
- **Idempotent writes.** A write may carry an `op_id`. Its acknowledgement (revision, parent, merged-from revision, operation count, and the nodes written and created) is stored in the commit's own transaction for seven days, bound to the caller and a digest of the request. A repeat of the same request by the same user, over MCP or HTTP, returns the stored acknowledgement marked as replayed instead of applying again, also when two calls race; a different request under a used `op_id` is refused as `op_id_mismatch`.
- **Locks.** A lease held by another holder (bridge, editor or agent) refuses the commit.
- **Size.** Uploads are bounded by `maxUploadBytes`, and requests by the input limits (see Configuration); a request over a limit is refused as `too_large` (HTTP 413).

Each commit updates the per-document search index (full-text over the extracted plain text). Revisions, presence and locks emit `document.revision.created`, `document.presence`, `document.lock` and `document.conflict` events on the AgUi stream (`/api/v1/stream/agui`), delivered to the owner and to users with presence on the document who still hold `view` or better when the event is emitted. Comments emit `document.comment.created` and `document.comment.resolved` to the same users. A grant emits one `document.shared` per recipient: the user it names, or each active member of the role it names (at most 500, the truncation logged), never the sharer. Each carries `recipient_user_id`, `document_name`, `shared_by` and `level`. An ownership transfer emits `document.ownership_transferred` to the new owner, the previous owner and present users. Clients filter on the `document.` prefix.

## MCP tools

Document tools are exposed through MCP and authorise as the calling user, never as the server. There are sixteen tools. Read-only tools are annotated as such.

| Tool | Purpose |
|---|---|
| `documents_list` | Documents by scope: one folder with its subfolders, documents shared with the caller, the caller's own, or the ones the caller last edited, commented on or opened. Filtered by name, sorted by name, last update or last touch, paged by cursor. Each item carries its size and the caller's access level. |
| `documents_search` | Full-text search over the head revision of every visible document. |
| `documents_create` | Create from a template, optionally applying edit operations in the same first revision. |
| `documents_read` | A revision as hashlines, by outline, text, node range, one node, spreadsheet cells or search; the changes since a revision; or the document's metadata and who is in it now. |
| `documents_edit` | Apply edit operations as one revision on the head, guarded per node, optionally idempotent by `op_id`. `answering` records the revision on a comment; an edit that removes text a comment is anchored to is refused unless `allow_detach` is set. |
| `documents_comment` | Create, list and resolve comment threads anchored to a node and, optionally, a quote inside it. |
| `documents_history` | Revisions, newest first, with `author`, `actor`, `at` (to the second) and message, and with `verbose` the session, context, trace, task and on-behalf-of user. |
| `documents_proposals` | A document's proposals with their status, or, with `action: accept` or `reject`, review one with an optional note (needs edit access). |
| `documents_share` | Grant a subject access to a document or a folder subtree with an optional `message`, transfer a document's ownership, or revoke every grant on it. |
| `documents_access` | The owner, the grants on a document with each subject's display name, the grants it inherits from its folders with the folder named, and the caller's level; for holders of `share`, open access requests and, with `history: true`, the grant audit log. |
| `documents_copy` | A new document owned by the caller from another document's head, with its own history. |
| `documents_update` | Rename a document or move it to another folder; `folder_id: null` moves it to the root. |
| `documents_trash` | Move a document to the trash. |
| `documents_untrash` | Bring a trashed document back. |
| `documents_export` | A signed ten-minute download link, or the sanitised HTML preview for the document-preview app. |
| `documents_guide` | Working detail by topic: `editing`, `formats`, `sheets`, `slides`, `sharing`, `comments`, `lifecycle`, `proposals` and `refusal.<code>`. |

The server `instructions` carry the working contract: read the outline, edit by node id and hash or by quote with an `op_id`, anchor the next edit on the acknowledgement and resume later with the changes since your last revision, let the head edit win on a conflict and never resend the old text, and do not re-read between your own writes.

**Reading.** A line is `<node>:<hash>| <text>`; a cell is `<sheet>!<cell>:<hash>| <value>`. The text shows structure as the outline does: `# ` by heading level, `- ` for a list item, `| ` for a table cell and `## slide N` at each slide. The outline folds a run of list items after three as `gap blocks=N`. `view: node` reads one node and its children, `view: search` the blocks containing a text, `view: meta` the name, place, access, head, size and who is present, and `view: changes` with `since_revision` the blocks changed since that revision, a `gap blocks=N` line for each run of unchanged blocks and a `gone <id>` line for each deleted block.

**Editing.** Operations are `replace_text`, `insert_after`, `delete`, `str_replace`, `set_cells`, `insert_slide`, `set_text`, `set_html`, `append_markdown`, `restore` and `insert_from_revision`. Text documents (`html`, `markdown`, `docx`, `odt`) also take the rich-text and table ops `set_marks`, `set_block_type`, `insert_table`, `table_insert_row`, `table_insert_col`, `table_delete` and `insert_image` (HTML and Markdown only). A batch of these applies through the document's `RichText` model, is validated like a live update, and may not mix with other ops; every block it does not change keeps its native markup. In `docx`, links are written as `w:hyperlink` elements with an external relationship. Every target in a call resolves against the document as it stood before the call, operations apply in order, and all apply or none. Each codec reports the node kinds `insert_after` accepts for its format. Every write takes a revision message, which history shows.

**Placeholders.** A `pending` block stands for content still to be written and carries its intent as text, so a document can be laid out as a skeleton and filled section by section. `insert_after` takes kind `pending`, and in Markdown content a line `<!-- pending: intent -->` is a pending block. Each format stores it natively: a highlighted run reading `[pending: intent]` in Word, a text paragraph in PowerPoint and OpenDocument, a cell in a spreadsheet, a `<p data-pending="intent">` in HTML, the comment followed by the intent in italics in Markdown, and a `[pending: intent]` line in text. A read shows it as kind `pending`; `replace_text` on it writes ordinary content in its place.

**Comments.** A comment is anchored to a stable node id and, optionally, to the nth occurrence of a quote inside that node; a reply carries its parent's anchor. Edits elsewhere never move an anchor. An edit that deletes the node or the quoted words is refused with `anchors_affected`, listing the comments, unless it sets `allow_detach`; a detached comment keeps its node and drops its quote. Bridge uploads and whole-file writes detach without refusing. Commenting needs `comment` access.

**Lifecycle.** A trashed document leaves listings, and reads, edits and shares refuse it as `trashed`; only a caller who could reach it learns it is trashed. Trashing and untrashing are the owner's. A document untrashed into a deleted folder lands at the root, and a name taken in the meantime gains the first free ` (restored)`, ` (restored 2)`, … suffix; a copy without a name takes the first free ` (copy)`, ` (copy 2)`, … name in its folder. Trashed documents are deleted, with their revisions, after the retention period. Transferring ownership is the owner's, and the previous owner keeps `edit`.

**Results.** A result is a one-line summary plus structured content, sent once. Its `_meta` carries only `io.systemprompt/execution` with the `artifact_id` and `mcp_execution_id` a client correlates on (and the UI resource URI for a host that renders the preview app). A one-op edit stays under 400 bytes and a three-revision history under 800 bytes on the full wire. Outputs are terse by default; `verbose: true` on a read, edit, history or list adds `document://<document_id>@<revision_id>` URIs, content hashes, each node id's positional path and the revision attribution. A write acknowledges the lines it changed or created, so the next edit anchors on them without a read; a write landed on a moved head says `merged`, and a replayed `op_id` says `replayed`.

**Refusals.** A refused call is a tool result with `isError: true`. Its structured content carries a stable `code` and the detail the failure knows: the head revision, its content hash, the base sent and who wrote the head, a node's current text and hash, a match count, the accepted node kinds, the owner to ask for access, the comments an edit would detach, or that a document is in the trash. The text names what to do next. The HTTP API answers in the same vocabulary (see [Errors](#errors)).

| Code | Meaning |
|---|---|
| `stale_base` | The base is no longer the head. |
| `guard_mismatch` | A touched node changed since it was read. |
| `find_none` | A quote matched no place, or more than one. |
| `unsupported_op` | The format does not take the operation or node kind. |
| `unsupported_format` | No codec handles the format. |
| `template_failed` | The template could not be instantiated. |
| `forbidden` | The caller lacks the access the action needs. |
| `not_found` | The document, folder, revision, comment or node does not exist. |
| `trashed` | The document is in the trash. |
| `locked` | Another holder has the lock. |
| `folder_not_empty` | The folder still holds documents or folders. |
| `anchors_affected` | The edit removes text comments are anchored to. |
| `proposal_conflict` | The proposal no longer applies at the head. |
| `proposal_closed` | The proposal was already reviewed or superseded. |
| `invalid_params` | The call is malformed. |
| `unknown_subject` | No active subject has that id. |
| `too_large` | The call exceeds an input limit. |
| `integrity_mismatch` | Uploaded bytes do not hash to the declared hash. |
| `op_id_mismatch` | The op id was used for a different request. |
| `self_review` | The reviewer proposed it. |

**Proposals.** An edit either lands on the head or is written as a proposal: a revision whose parent is its base and which never becomes the head on its own. `documents_edit` and `POST /edits` take `mode: land | propose`. Without a mode, a caller holding `comment` but not `edit` access proposes, an agent proposes in a folder whose agent edit policy is `propose` (set on the folder or inherited from any folder above it; the root is `direct`), and everyone else lands. `mode: land` needs `edit` access, and an agent's `mode: land` in a `propose` folder is refused as `forbidden` with the policy named, since the folder policy outranks the request. The acknowledgement carries the `proposal_id`, the proposed revision and its lines against the base. A newer proposal from the same proposer on the same document supersedes the open one.

Accepting needs `edit` access, and nobody accepts their own proposal (`self_review`). When the head is still the proposal's base, the proposed revision becomes the head by compare-and-swap and keeps the proposer as its author. When the head has moved, the proposal's operations are applied again on the head under the same node guards as any edit, as a revision by the reviewer on behalf of the proposer; a node the proposal touches that changed in between refuses as `proposal_conflict` with its current text, and the proposal stays open. The landing transaction records the landed revision on the proposal, so an accepted proposal always names its revision; an accept interrupted before it landed is reopened by the presence sweep after five minutes. Rejecting records the reviewer and note. Every transition emits `document.proposal.opened`, `accepted`, `rejected` or `superseded` to the owner, present users and the proposer. Moving a document to the trash rejects its open proposals.

**Export.** `native` as a URL works for every format, and `html` as a preview works for every format. `md` or `html` as a URL is offered only for a document already in that format, and `pdf` is refused as `unsupported_format`.

The tool handlers are composed by an extension's MCP server binary; core provides the handlers and the service behind them.

## HTTP API

Routes mount under `/api/v1/documents` and `/api/v1/document-folders` with user authentication.

| Route | Purpose |
|---|---|
| `GET`, `POST /api/v1/documents` | List by `scope` (`folder`, `shared`, `mine`, `recent`), `folder`, `query` and `sort` (`name`, `updated`, `recent`), with each document's `size_bytes` and `my_access`; create from a template, or import by multipart upload. |
| `GET`, `HEAD`, `PATCH`, `DELETE /api/v1/documents/{id}` | Metadata, rename and move, and trash. The `ETag` is the head revision. |
| `POST /api/v1/documents/{id}/untrash`, `/copy` | Bring back from the trash; copy into a new document. |
| `GET /api/v1/documents/{id}/read` | The `documents_read` views, assembled by the same code: `view=outline`, `text`, `range` (`from_node`, `to_node`), `cells` (`sheet`, `cells`), `changes` (`since_revision`), `node` (`node_id`), `search` (`text`) or `meta`, at `revision_id` or the head; `verbose=true` as on the tool. |
| `GET`, `POST /api/v1/documents/{id}/comments`, `POST /api/v1/documents/{id}/comments/{comment_id}/resolve` | Comment threads. |
| `GET /api/v1/documents/{id}/access` | Owner, grants with display names, inherited grants with `inherited_from`, the caller's level, open access requests for holders of `share`, and with `?history=true` the grant audit log. |
| `POST /api/v1/documents/{id}/transfer`, `DELETE /api/v1/documents/{id}/grants/all` | Ownership transfer; revoke every grant. |
| `GET /api/v1/documents/subject-kinds` | The grant subject kinds this installation accepts. |
| `GET /api/v1/documents/subjects` | Subject search by `kind`, `q` (prefix) and `limit` (at most 20). |
| `POST /api/v1/documents/{id}/access-requests`, `POST /api/v1/documents/{id}/access-requests/{rid}/grant`, `/decline` | Request access; answer a request (needs `share`). |
| `GET`, `PUT /api/v1/documents/{id}/content` | Download with range and conditional support. Replace the bytes with `If-Match: "<revision_id>"`; a stale base merges as described above or returns `409` `stale_base` with the head, its content hash and the base under `detail`. `?op_id=` makes the write idempotent. |
| `POST /api/v1/documents/{id}/edits` | Structured edit operations, with an optional base revision, `op_id`, `answering`, `allow_detach` and `mode` (`land` or `propose`). A proposal answers `201` with the proposal and its revision. |
| `GET /api/v1/documents/{id}/proposals` | Proposals, newest first; `?status=` filters. |
| `GET /api/v1/documents/{id}/proposals/{pid}` | One proposal and the lines it changes against its base, in the changes-view form. |
| `POST /api/v1/documents/{id}/proposals/{pid}/accept`, `/reject` | Review with an optional `note`. Accept answers the landed revision and whether it was rebased. |
| `GET /api/v1/documents/{id}/revisions` | Revision history. |
| `GET /api/v1/documents/{id}/projection`, `/preview` | Node projection and HTML preview. |
| `GET`, `POST`, `DELETE /api/v1/documents/{id}/grants` | Grants; `POST` takes an optional `message`. |
| `GET`, `POST`, `DELETE /api/v1/document-folders/{id}/grants`, `GET /api/v1/document-folders/{id}/access` | Folder grants and access, in the same shapes as a document's. |
| `PUT`, `GET /api/v1/documents/{id}/presence` | Presence beacons. |
| `POST`, `DELETE /api/v1/documents/{id}/lock` | Lease lock. |
| `GET`, `POST`, `PATCH /api/v1/document-folders` | Folders: list, create, rename, move, and set `agent_edit_policy` (`direct` or `propose`, needs `share` access). |

With the `files-collab` facade feature (part of `full`), four further routes co-edit HTML and Markdown documents live. See [Live co-editing](#live-co-editing).

A write records the request's session, trace and task, and the conversation named by an `x-context-id` header. A merged write answers `merged_from`, and a replayed `op_id` answers the stored response with `replayed: true`.

### Errors

A refused request answers the common error envelope ([reference/http-api.md](reference/http-api.md)) with the refusal code of the MCP tools as its `error_key` and the same typed detail under `detail`, so one client handles both surfaces. A server-side failure answers a fixed message; its cause is logged, never returned. When the install enables access requests, a signed-in caller's `not_found` on an existing document also carries `can_request: true` under `detail`.

| Status | Codes |
|---|---|
| `409` | `stale_base`, `guard_mismatch`, `anchors_affected`, `find_none`, `proposal_conflict`, `proposal_closed`, `folder_not_empty`, `op_id_mismatch`, `access_request_open`, `access_request_closed`, and `not_found` for a node id |
| `403` | `forbidden`, `self_review` |
| `404` | `not_found`, `trashed` |
| `413` | `too_large` |
| `422` | `integrity_mismatch`, `template_failed`, and `invalid_params` for a document that cannot be parsed |
| `423` | `locked` (a held lock also names the `lock` under `detail`) |
| `400` | `invalid_params`, `unsupported_op`, `unsupported_format`, `unknown_subject`, `access_request_not_needed`, `find_none` for an `old_str` quote |

## Live co-editing

HTML and Markdown documents can be edited by several people at once. A session holds the document as core's `RichText` model (`systemprompt_files::documents::model`): an ordered list of blocks, each with a stable `id`, a `type` (`paragraph`, `heading` 1–6, `list_item` with `level` and `ordered`, `quote`, `code`, `table` of cells that are themselves `RichText`, `image`, `divider`, `page_break`, or `raw` for native markup the model does not own) and text carrying `bold`, `italic`, `underline`, `strike`, `code`, `link`, `color`, `highlight` and `comment` marks over Unicode scalar offsets. HTML is read from the inner markup of `<body>` (the whole document when it has no `<body>`) and Markdown from the full text; element types the model does not own are kept verbatim as `raw` blocks, so they survive every checkpoint. `rich_text_schema()` returns the model's JSON Schema. The model is held by the installation's document engine, `systemprompt-engine-loro` ([Loro](https://loro.dev)) behind the facade's `files-collab-loro` feature: a movable list named `blocks` of maps (`id`, `kind` as JSON, `text` as rich text), with the mark expansion of `systemprompt_engine_loro::rich::TEXT_STYLES`, which every client must configure. With the `files-office` feature, `docx` and `odt` documents are co-edited too: each top-level paragraph of the body is a block (headings from `HeadingN`/`Title` styles or `text:h`, list items from `w:numPr`; in `docx` the runs' bold, italic and underline are marks), while tables, paragraphs holding a drawing or note, and every other top-level element are `raw` blocks. A checkpoint writes an unchanged block as its original bytes and generates only new or edited paragraphs, keeps every byte between blocks, and rewrites only `word/document.xml` or `content.xml`, so every other package part is byte-identical. Spreadsheets and decks are not co-edited and answer `400`. With `files-collab` and no engine, every session route answers `503` with `collab_engine_unavailable`.

| Route | Body | Response |
|---|---|---|
| `POST /api/v1/documents/{id}/collab/join` | `{"mode": "edit" \| "comment" \| "view"}` | `{protocol, session_id, snapshot_b64, vv, seq, head_revision, engine, wire_version, model, model_version, level}` |
| `POST /api/v1/documents/{id}/collab/updates` | `{session_id, vv?, update_b64}`, or raw bytes with `Content-Type: application/octet-stream` and `?session_id=…&vv=…` | `{seq, vv, missing_b64?}` |
| `POST /api/v1/documents/{id}/collab/leave` | `{session_id}` | `{closed, checkpoint, checkpoint_revision, head_revision?, proposal_id?}` |
| `POST /api/v1/documents/{id}/collab/cursor` | `{session_id, cursor}` | `204` |

Binary values are standard base64: `snapshot_b64` is a Loro snapshot, `update_b64` a Loro update, `vv` a Loro version vector. `edit` needs edit access, `comment` comment access and `view` view access; join answers protocol `2`, the engine and `wire_version` the client must speak, the `model` (`rich_text`) and its `model_version`, and the caller's access `level`. Updates are accepted only from a user who joined in edit or comment mode (`403` otherwise). Each update is applied to a fork of the server's own copy and the result is checked before it is logged: block, mark and link-scheme allowlists, size and nesting limits, unique block ids, `raw` blocks that only the server may author, and the caller's level — a comment-level caller may change comment marks and nothing else. A refused update answers `400` with `update_rejected` and a `reason` (`too_large`, `bad_block_id`, `bad_block`, `bad_mark`, `raw_block_changed`, `beyond_access_level`); it is never logged or broadcast, and the client resyncs from a fresh join. An update for a session that is no longer open answers `409`, and the client joins again. When the posted `vv` lacks changes the server holds, `missing_b64` carries them.

A document has at most one open edit session. Every accepted update is logged with a sequence number and sent as a `document.collab.update` event (`document_id`, `edit_session_id`, `seq`, `update_b64`, `author_id`) on the AG-UI stream of each user present on the document; there is no separate stream. Sequence numbers are per session, consecutive and assigned in commit order, starting after the `seq` returned by join; a client that receives an event whose `seq` skips one joins again and imports the new snapshot. An update is logged before any replica applies it, so a refused or failed update changes nothing. Joins and cursor moves send `document.presence`, with `cursor` when one was posted.

The server stores a snapshot 30 seconds after the first unsaved update or after 200 updates, and commits the body as a revision with origin `edit_session` after 2 minutes of activity and when the last editor leaves, which also closes the session. Checkpoints are attributed to the session's last editor; a viewer leaving never commits. `checkpoint` on leave is `unchanged`, `committed` (with `checkpoint_revision`) or `conflict` (with `head_revision`). On `conflict` the live body is kept as a proposal by the last editor (`proposal_id`) and the session stays open. Moving a document to the trash commits its live body the same way, closes its edit session and clears its locks and presence. For HTML the revision keeps the document around `<body>` from the revision the checkpoint is based on. A checkpoint that the revision engine merges with writes made outside the session re-imports the merged document, carries the live block ids over to the matching blocks, and writes it back into the live model. A session with no edits never commits: a checkpoint compares the materialised model with the materialised import of its base, so the normalisation a re-render applies is not mistaken for an edit. A session opened before the model existed still holds a flat `body` text and checkpoints as before.

### Extension pages

While `files.documents.enabled` is on, the extension `Dependencies` carry `Arc<DocumentService>`; an extension router that serves a document page reads it with `ctx.dependencies().get::<Arc<DocumentService>>()` and authorizes the request through the service before rendering. With the store off the entry is absent, and a page that needs it is not mounted.

## Web components

The files extension ships native ES module Web Components for browsing, previewing and editing documents. The generator's asset job copies them into the web distribution:

| Path | Content |
|---|---|
| `assets/documents/js/documents.js` | Entry module; importing it defines every component |
| `assets/documents/js/*.js` | One module per component, plus the HTTP client, the event stream and helpers |
| `assets/documents/css/documents.css`, `documents-panels.css`, `documents-sharing.css` | Styles built on the install's `--sp-` design tokens |

Core ships no third-party frontend code, so there is no live editor component: a live-session client vendors its own CRDT client at the engine's wire version (`systemprompt_engine_loro::WIRE_VERSION`) and speaks the collab routes above.

| Element | Attributes | Uses |
|---|---|---|
| `sp-document-browser` | `scope` (`mine`, `shared`, `recent`) | `GET /documents` (folder navigation, scope and name filter); dispatches `sp-document-open` with `{id, format, name}` |
| `sp-document-preview` | `document-id`, `revision` | `GET /{id}/preview` in a sandboxed frame without scripts, `PUT /{id}/presence` |
| `sp-presence-bar` | `document-id` | `GET /{id}/presence`, `document.presence` events |
| `sp-sheet-grid` | `document-id` | `GET /{id}/projection`, `POST /{id}/edits` with `set_cells` |
| `sp-slide-text` | `document-id` | `GET /{id}/projection`, `POST /{id}/edits` with `replace_text` by node id and hash |
| `sp-share-dialog` | `document-id` | `GET /{id}/access` (with `history` when the history panel is open) and `GET /subject-kinds`, `POST` and `DELETE /{id}/grants`, `DELETE /{id}/grants/all`, `POST /{id}/transfer` and the access-request answers; below `share` it shows who has access read-only; transfer and revoke-all ask for confirmation |
| `sp-subject-picker` | `kinds`, `label` | `GET /subjects` as a debounced, keyboard-navigable typeahead inside the share and transfer forms |
| `sp-history` | `document-id` | `GET /{id}/revisions`, a `restore` op on `POST /{id}/edits`, and `GET /{id}/read?view=changes` since a revision |
| `sp-comments` | `document-id`, `anchor-node` | `/{id}/comments`; hidden when the install answers `404` |
| `sp-proposal-list` | `document-id` | `GET /{id}/proposals`, the changed lines from `GET /{id}/proposals/{pid}`, accept or reject with a note, `document.proposal.*` events; hidden when the install answers `404` |

Every edit carries a fresh `op_id`, so a retried request replays its first acknowledgement. A refusal is shown with its error key. All components on a page share one AG-UI stream (`/api/v1/stream/agui`), because each user may hold at most ten streams.

The asset sources resolve inside the `systemprompt-files` crate, so they are declared optional: a build that compiles core copies them, and a runtime without the crate's sources keeps the copies already in its web distribution.

## Bridge Drive sync

The bridge keeps `~/<workspace>/Drive` in step with the store. It talks to six endpoints under `/v1/bridge/drive/`, authenticated like the bridge heartbeat.

| Endpoint | Purpose |
|---|---|
| `GET manifest` | Every document the user can view, with head revision and hash. |
| `GET changes?since=` | Revisions, trashes, restores, renames and moves since a keyset cursor, in one ordered stream. |
| `GET blob/{sha256}` | Bytes by hash. The bridge verifies the hash before it writes. |
| `POST commit` | Multipart metadata and bytes with `If-Match` set to the base revision. Returns the new revision, the merged revision and its hash when the upload landed on a newer head, or a conflict naming the head. |
| `PUT presence` | Editing beacons. |
| `GET proposals` | Open proposals on the user's documents and the user's own open proposals. |

Behaviour:

- A watcher debounces local writes by 1.5 seconds, ignores dot files, `~$` lock files, backup `~` files and `.tmp` or `.crdownload` partials, and commits the file as a revision.
- A pull loop polls the change feed every 20 seconds, or sooner when a `document.revision.created` event arrives on the bridge stream. Remote bytes are hash-verified and written atomically.
- A merged upload is recorded against the uploaded bytes, and the next pull writes the merged content.
- A conflicting local edit is renamed to `<name> (conflict from <host>).<ext>` and the head is pulled, so neither version is lost.
- A document trashed remotely, alone or with its folder, moves into `Drive/.trash/<timestamp>/`. A restored document is pulled again at the path it returned to. A local deletion is not propagated.
- A document renamed or moved to another folder remotely is moved to its new path, with its sync state. A folder rename or move reaches the device when it next reconciles against the manifest.
- The local file is hashed again just before a downloaded head replaces it. An edit made while the blob was downloading is kept as a conflict sibling.
- Presence is reported from file activity and from agent tool calls that touch a path inside the Drive folder.
- A `document.proposal.opened` event on the bridge stream raises a desktop notification, and so does a `document.shared` event whose `recipient_user_id` is the device user, naming the document, the sharer and the share message when there is one.
- Each sync refreshes the open proposals, which `bridge drive status` lists with the documents shared with the user that the folder has not synced yet.
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
| `accessRequests` | `false` | | Lets a signed-in user with no level request access, and adds `can_request: true` to their `not_found` refusals. |
| `limits.maxOpsPerCall` | 200 | 1 to 10,000 | Edit operations in one call. |
| `limits.maxOpTextBytes` | 1 MiB | 1 KiB to 64 MiB | Text one operation carries. |
| `limits.maxMarkdownBytes` | 1 MiB | 1 KiB to 64 MiB | Markdown, HTML or whole-document text one operation carries. |
| `limits.maxCommentBytes` | 16 KiB | 256 B to 1 MiB | A comment body. |
| `limits.maxCollabUpdateBytes` | 4 MiB | 1 KiB to 64 MiB | One live co-editing update. |
| `limits.maxCursorBytes` | 4 KiB | 256 B to 64 KiB | One editor cursor payload. |

Config validation rejects values outside the bounds.

## Background jobs

| Job | Schedule | Behaviour |
|---|---|---|
| `document_blob_gc` | daily | Deletes blobs that no revision or edit-session snapshot references and that are older than the grace period, in batches of 500. |
| Presence sweep | every 60 seconds | Removes expired presence rows, grants past their `valid_until` (with the named user's presence on that document, and an `expired` audit row) and write acknowledgements older than seven days, and reopens accepted proposals that have not landed after five minutes. |
| `document_collab_update_sweep` | every 5 minutes | Deletes the update log of closed edit sessions. |
| `document_node_ids_backfill` | every 10 minutes, once per release | Mints stable node ids for revisions written before ids existed, oldest first so a child carries its parent's ids. Reads never mint: until then such a revision refuses `not_found`. It runs once per release across the cluster under a release stamp. |
| `document_trash_purge` | daily | Deletes documents trashed longer than the retention period, with their revisions, node ids, grants and comments; the blob GC then collects their blobs. |

All run once per cluster, and return without work while the store is disabled.

## Planned

These items are designed and not part of the current release: the Web Components for browsing and editing, including the browser editor for live co-editing. Browser access today is the HTTP API.
