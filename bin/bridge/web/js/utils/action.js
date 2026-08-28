// Runs one user-initiated action under the contract this app did not have:
// a pending state on the control, then exactly one of a success confirmation
// or an error carrying a next step. Never silence.

import { notifyOk, notifyErr } from "/assets/js/utils/notify.js";

function setBusy(trigger, busy) {
  // The reconciler can replace the trigger between the press and the reply, so
  // every touch of it is guarded — writing to a detached node was how the old
  // `trigger.title = e.message` lost its message even to a hover.
  if (!trigger || !trigger.isConnected) { return; }
  if (busy) {
    trigger.setAttribute("aria-busy", "true");
    trigger.disabled = true;
  } else {
    trigger.removeAttribute("aria-busy");
    trigger.disabled = false;
  }
}

/**
 * @param {Element|null} trigger the control that was pressed, or null
 * @param {{run: () => Promise<any>, success?: string|((v: any) => string), context: string}} opts
 * @returns {Promise<{ok: boolean, value?: any, error?: any}>}
 */
export async function runAction(trigger, { run, success, context }) {
  setBusy(trigger, true);
  try {
    const value = await run();
    const line = typeof success === "function" ? success(value) : success;
    if (line) { notifyOk(line); }
    return { ok: true, value };
  } catch (error) {
    notifyErr(error, context);
    return { ok: false, error };
  } finally {
    setBusy(trigger, false);
  }
}
