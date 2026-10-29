/*
 * The only module that talks HTTP. Every route the document components use
 * has one helper here; a refusal is thrown as a DocumentsApiError carrying the
 * server's stable `code` (the envelope's `error_key`, else its `code`) so a
 * component can branch on it instead of parsing a message.
 */
const DOCUMENTS = "/api/v1/documents";
const FOLDERS = "/api/v1/document-folders";

export class DocumentsApiError extends Error {
  constructor(status, body) {
    super(body?.message ?? `request failed (${status})`);
    this.status = status;
    this.code = body?.error_key ?? body?.code ?? String(status);
    this.body = body;
  }
}

const query = (params) => {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params ?? {})) {
    if (value != null && value !== "") { search.set(key, String(value)); }
  }
  const text = search.toString();
  return text ? `?${text}` : "";
};

const readBody = async (res) => {
  if (res.status === 204) { return null; }
  const type = res.headers.get("content-type") ?? "";
  if (type.includes("json")) { return res.json(); }
  return res.text();
};

const request = async (method, url, body) => {
  const init = { method, credentials: "same-origin", headers: { accept: "application/json" } };
  if (body !== undefined) {
    init.headers["content-type"] = "application/json";
    init.body = JSON.stringify(body);
  }
  const res = await fetch(url, init);
  const payload = await readBody(res);
  if (!res.ok) { throw new DocumentsApiError(res.status, typeof payload === "object" ? payload : { message: payload }); }
  return payload;
};

const doc = (id, tail = "") => `${DOCUMENTS}/${encodeURIComponent(id)}${tail}`;

export const opId = () => crypto.randomUUID();

export const listDocuments = (params) => request("GET", `${DOCUMENTS}/${query(params)}`);
export const getDocument = (id) => request("GET", doc(id));
export const listFolders = (params) => request("GET", `${FOLDERS}/${query(params)}`);
export const previewUrl = (id, revision) => doc(id, `/preview${query({ revision })}`);

export const getProjection = (id, params) => request("GET", doc(id, `/projection${query(params)}`));
export const listRevisions = (id, params) => request("GET", doc(id, `/revisions${query(params)}`));

export const postEdits = (id, { baseRevision, ops, message, opId: editId }) =>
  request("POST", doc(id, "/edits"), { base_revision: baseRevision, ops, message, op_id: editId });

export const restoreRevision = (id, { revisionId, headRevision, opId: editId }) =>
  postEdits(id, { baseRevision: headRevision, ops: [{ op: "restore", revision_id: revisionId }], message: "restore", opId: editId });

export const readView = (id, params) => request("GET", doc(id, `/read${query(params)}`));
const folder = (id, tail = "") => `${FOLDERS}/${encodeURIComponent(id)}${tail}`;

export const getAccess = (id, { history } = {}) => request("GET", doc(id, `/access${query({ history })}`));
export const getFolderAccess = (id, { history } = {}) => request("GET", folder(id, `/access${query({ history })}`));
export const listSubjectKinds = () => request("GET", `${DOCUMENTS}/subject-kinds`);
export const searchSubjects = ({ kind, q, limit }) => request("GET", `${DOCUMENTS}/subjects${query({ kind, q, limit })}`);
export const transferOwnership = (id, userId) => request("POST", doc(id, "/transfer"), { transfer_to: userId });
export const revokeAllGrants = (id) => request("DELETE", doc(id, "/grants/all"));
export const shareDocument = (id, { subjectKind, subjectValue, level, validUntil, message }) =>
  request("POST", doc(id, "/grants"), {
    subject_kind: subjectKind,
    subject_value: subjectValue,
    level,
    valid_until: validUntil || undefined,
    message: message || undefined,
  });
export const revokeGrant = (id, grantId) => request("DELETE", doc(id, `/grants${query({ grant_id: grantId })}`));
export const requestAccess = (id, { level, message }) =>
  request("POST", doc(id, "/access-requests"), { level, message: message || undefined });
export const decideAccessRequest = (id, requestId, decision) =>
  request("POST", doc(id, `/access-requests/${encodeURIComponent(requestId)}/${decision}`));

export const getPresence = (id) => request("GET", doc(id, "/presence"));
export const touchPresence = (id, state) => request("PUT", doc(id, "/presence"), { state });

export const collabJoin = (id, mode) => request("POST", doc(id, "/collab/join"), { mode });
export const collabUpdate = (id, { sessionId, vv, updateB64 }) =>
  request("POST", doc(id, "/collab/updates"), { session_id: sessionId, vv, update_b64: updateB64 });
export const collabLeave = (id, sessionId) => request("POST", doc(id, "/collab/leave"), { session_id: sessionId });
export const collabLeaveOnUnload = (id, sessionId) => fetch(doc(id, "/collab/leave"), {
  method: "POST",
  credentials: "same-origin",
  keepalive: true,
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ session_id: sessionId }),
});
export const collabCursor = (id, sessionId, cursor) =>
  request("POST", doc(id, "/collab/cursor"), { session_id: sessionId, cursor });

export const listProposals = (id, status) => request("GET", doc(id, `/proposals${query({ status })}`));
export const getProposal = (id, proposalId) => request("GET", doc(id, `/proposals/${encodeURIComponent(proposalId)}`));
export const decideProposal = (id, proposalId, decision, note) =>
  request("POST", doc(id, `/proposals/${encodeURIComponent(proposalId)}/${decision}`), { note });

export const listComments = (id) => request("GET", doc(id, "/comments"));
export const postComment = (id, { anchor, body, parentId }) =>
  request("POST", doc(id, "/comments"), { anchor, body, parent_id: parentId });
export const resolveComment = (id, commentId) =>
  request("POST", doc(id, `/comments/${encodeURIComponent(commentId)}/resolve`));

export const STREAM_URL = "/api/v1/stream/agui";
