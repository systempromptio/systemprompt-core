// The single seam between "something happened" and the user being told.
//
// The app has always had a toast — `sp-toast.js` and its info/error/success
// styling — wired to the Rust error channel and called by nothing else, so
// every failure originating in the front end was delivered as a `title`
// attribute on the button that had just been pressed. This module is what the
// rest of the app calls instead.
//
// It dispatches a DOM event rather than reaching for the element, so nothing
// has to know where the toast lives or whether it is mounted yet.

/**
 * Bridge rejections are plain `BridgeError` objects, not `Error` instances, so
 * `e.message` is present but `String(e)` is useless on its own.
 */
export function errorText(e) {
  if (!e) { return "unknown error"; }
  if (typeof e === "string") { return e; }
  return e.message || e.code || String(e);
}

/**
 * `key` is what de-duplication compares. It defaults to the message, but an
 * error passes the raw failure text so that the same failure arriving twice —
 * once as a rejected request, once on the Rust error channel — is recognised
 * despite the two lines being worded differently.
 */
export function notify(message, kind = "info", durationMs, key) {
  document.dispatchEvent(new CustomEvent("sp:toast", {
    detail: { message, kind, durationMs, key },
  }));
}

export function notifyOk(message) {
  notify(message, "success", 4000);
}

/**
 * `context` names the action in the user's vocabulary — "Sync", "Save filter" —
 * so the toast reads as a report on what they pressed rather than as a stray
 * system message.
 */
export function notifyErr(e, context) {
  const text = errorText(e);
  notify(context ? `${context}: ${text}` : text, "error", 8000, text);
}

/** Stays until dismissed: the user has to go and do something themselves. */
export function notifyAction(message) {
  notify(message, "info", 0);
}
