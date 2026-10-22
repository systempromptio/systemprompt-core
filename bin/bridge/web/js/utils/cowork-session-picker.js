// The fix for "several Cowork org sessions are usable": list them and pin the
// one the person picks as `cowork.session_org_dir`. The bridge only accepts a
// path it listed itself, then re-syncs so the org plugins land there.

import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) { node.className = className; }
  if (text !== undefined) { node.textContent = text; }
  return node;
}

function sessionRow(session, choose) {
  const button = el("button", "sp-btn sp-cowork-picker__choice");
  button.type = "button";
  button.append(
    el("span", "sp-cowork-picker__org", session.org),
    el("span", "sp-cowork-picker__path", session.path),
  );
  button.addEventListener("click", () => choose(session.path));
  return button;
}

/**
 * Open the picker. Resolves true once a session is pinned, false when the
 * person closes it without choosing.
 */
export async function pickCoworkSession() {
  const { sessions = [] } = (await bridge.coworkSessions()) || {};
  const dialog = el("dialog", "sp-cowork-picker");
  dialog.setAttribute("aria-labelledby", "sp-cowork-picker-title");
  const title = el("h2", "sp-cowork-picker__title",
    t("cowork-picker-title") || "Choose your Cowork organization");
  title.id = "sp-cowork-picker-title";
  const hint = el("p", "sp-cowork-picker__hint",
    t("cowork-picker-hint")
      || "Claude Desktop has several Cowork sessions on this computer. Pick the one for this organization; the plugins are enabled there.");
  const list = el("div", "sp-cowork-picker__list");
  const cancel = el("button", "sp-btn-ghost", t("cowork-picker-cancel") || "Cancel");
  cancel.type = "button";
  dialog.append(title, hint, list, cancel);
  document.body.append(dialog);

  return new Promise((resolve, reject) => {
    const close = (chosen) => { dialog.close(); dialog.remove(); resolve(chosen); };
    const choose = async (path) => {
      try {
        await bridge.coworkChooseSession(path);
        close(true);
      } catch (e) {
        dialog.close(); dialog.remove(); reject(e);
      }
    };
    if (sessions.length === 0) {
      list.append(el("p", "sp-cowork-picker__hint",
        t("cowork-picker-empty") || "No Cowork session was found. Open Claude Desktop's Cowork tab once, then try again."));
    }
    for (const session of sessions) { list.append(sessionRow(session, choose)); }
    cancel.addEventListener("click", () => close(false));
    dialog.addEventListener("cancel", () => close(false));
    dialog.showModal();
  });
}
