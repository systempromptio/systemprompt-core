import { STREAM_URL } from "./documents-api.js";

/*
 * One EventSource per page. The server caps each user at ten SSE connections,
 * so every component subscribes here instead of opening its own stream. The
 * stream is the user's AG-UI feed; document events arrive as CUSTOM events
 * whose `name` is `document.*` and whose `value` carries `document_id`.
 */
const listeners = new Map();
let source = null;

const dispatch = (evt) => {
  let event;
  try { event = JSON.parse(evt.data); } catch { return; }
  if (event?.type !== "CUSTOM" || !String(event.name ?? "").startsWith("document.")) { return; }
  const documentId = event.value?.document_id;
  for (const key of [documentId, "*"]) {
    for (const fn of listeners.get(key) ?? []) { fn(event.name, event.value); }
  }
};

const open = () => {
  if (source) { return; }
  source = new EventSource(STREAM_URL, { withCredentials: true });
  source.addEventListener("message", dispatch);
};

const closeIfIdle = () => {
  if (listeners.size > 0 || !source) { return; }
  source.close();
  source = null;
};

export const subscribeDocument = (documentId, fn) => {
  const key = documentId ?? "*";
  const set = listeners.get(key) ?? new Set();
  set.add(fn);
  listeners.set(key, set);
  open();
  return () => {
    set.delete(fn);
    if (set.size === 0) { listeners.delete(key); }
    closeIfIdle();
  };
};
