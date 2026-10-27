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

const RETRY_DELAYS_MS = [250, 1000, 4000];

const wait = (ms) => new Promise((resolve) => { setTimeout(resolve, ms); });

const isTransient = (err) => !err?.status || err.status >= 500 || err.status === 429;

/*
 * The diff runs over code points, never UTF-16 units: a cut inside a surrogate
 * pair would send Loro half a character, and Loro text positions count code
 * points.
 */
const splice = (beforeText, afterText) => {
  const before = Array.from(beforeText);
  const after = Array.from(afterText);
  let start = 0;
  while (start < before.length && start < after.length && before[start] === after[start]) { start += 1; }
  let tail = 0;
  while (tail < before.length - start && tail < after.length - start
    && before[before.length - 1 - tail] === after[after.length - 1 - tail]) { tail += 1; }
  return { start, removed: before.length - start - tail, inserted: after.slice(start, after.length - tail).join("") };
};

const postWithRetry = async (send) => {
  for (const delay of RETRY_DELAYS_MS) {
    try {
      return await send();
    } catch (err) {
      if (!isTransient(err)) { throw err; }
      await wait(delay);
    }
  }
  return send();
};

export class CollabSession {
  constructor(documentId) {
    this.documentId = documentId;
    this.sessionId = null;
    this.doc = null;
    this.seq = 0;
  }

  async join(mode) {
    await loadWasm();
    this.mode = mode;
    const joined = await collabJoin(this.documentId, mode);
    this.sessionId = joined.session_id;
    this.doc ??= new LoroDoc();
    this.doc.import(fromBase64(joined.snapshot_b64));
    this.sent ??= this.doc.oplogVersion();
    this.seq = joined.seq;
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
    const version = this.doc.oplogVersion();
    const update = this.doc.export({ mode: "update", from: this.sent });
    const applied = await postWithRetry(() => collabUpdate(this.documentId, {
      sessionId: this.sessionId,
      vv: toBase64(version.encode()),
      updateB64: toBase64(update),
    }));
    this.sent = version;
    this.seq = Math.max(this.seq, applied.seq);
    if (applied.missing_b64) { this.doc.import(fromBase64(applied.missing_b64)); }
    return applied;
  }

  async importRemote(event) {
    if (event.edit_session_id !== this.sessionId) { return false; }
    if (event.seq > this.seq + 1) {
      await this.join(this.mode);
      return true;
    }
    this.doc.import(fromBase64(event.update_b64));
    this.seq = Math.max(this.seq, event.seq);
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
