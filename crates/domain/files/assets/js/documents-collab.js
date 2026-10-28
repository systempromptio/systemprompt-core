import init, { LoroDoc } from "../wasm/loro.js";
import { collabCursor, collabJoin, collabLeave, collabLeaveOnUnload, collabUpdate } from "./documents-api.js";

/*
 * One live edit session over the server's Loro document. The server keeps the
 * HTML or Markdown source in the text container named `body`; this side
 * mirrors it, turns each local change into a Loro update, and imports every
 * update the AG-UI stream delivers. Importing an update twice is a no-op in
 * Loro, so the echo of our own update needs no filtering.
 */
const BODY = "body";
let ready = null;

const loadWasm = () => {
  ready ??= init({ module_or_path: new URL("../wasm/loro.wasm", import.meta.url) });
  return ready;
};

export const toBase64 = (bytes) => {
  let text = "";
  for (const byte of bytes) { text += String.fromCharCode(byte); }
  return btoa(text);
};

export const fromBase64 = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

const splice = (before, after) => {
  let start = 0;
  while (start < before.length && start < after.length && before[start] === after[start]) { start += 1; }
  let tail = 0;
  while (tail < before.length - start && tail < after.length - start
    && before[before.length - 1 - tail] === after[after.length - 1 - tail]) { tail += 1; }
  return { start, removed: before.length - start - tail, inserted: after.slice(start, after.length - tail) };
};

export class CollabSession {
  constructor(documentId) {
    this.documentId = documentId;
    this.sessionId = null;
    this.doc = null;
  }

  async join(mode) {
    await loadWasm();
    const joined = await collabJoin(this.documentId, mode);
    this.sessionId = joined.session_id;
    this.doc = new LoroDoc();
    this.doc.import(fromBase64(joined.snapshot_b64));
    this.sent = this.doc.oplogVersion();
    return joined;
  }

  text() {
    return this.doc.getText(BODY).toString();
  }

  async push(next) {
    const body = this.doc.getText(BODY);
    const { start, removed, inserted } = splice(body.toString(), next);
    if (removed === 0 && inserted === "") { return null; }
    if (removed > 0) { body.delete(start, removed); }
    if (inserted) { body.insert(start, inserted); }
    this.doc.commit();
    const update = this.doc.export({ mode: "update", from: this.sent });
    this.sent = this.doc.oplogVersion();
    const applied = await collabUpdate(this.documentId, {
      sessionId: this.sessionId,
      vv: toBase64(this.sent.encode()),
      updateB64: toBase64(update),
    });
    if (applied.missing_b64) { this.doc.import(fromBase64(applied.missing_b64)); }
    return applied;
  }

  importRemote(event) {
    if (event.edit_session_id !== this.sessionId) { return false; }
    this.doc.import(fromBase64(event.update_b64));
    return true;
  }

  cursor(position) {
    return collabCursor(this.documentId, this.sessionId, position);
  }

  leave({ unloading = false } = {}) {
    if (!this.sessionId) { return null; }
    const sessionId = this.sessionId;
    this.sessionId = null;
    return unloading ? collabLeaveOnUnload(this.documentId, sessionId) : collabLeave(this.documentId, sessionId);
  }
}
