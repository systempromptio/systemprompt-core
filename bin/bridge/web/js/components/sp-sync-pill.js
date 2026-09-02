import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { announce } from "/assets/js/utils/announce.js";
import { runAction } from "/assets/js/utils/action.js";

// `overall` is the bridge's one-dot summary; the footer draws the same one.
function classify(snap) {
  const overall = snap.overall || { tone: "unknown", code: "needs-sign-in" };
  return { tone: overall.tone, text: t(`overall-${overall.code}`) || "" };
}

// What the current sync is doing, or "" when there is nothing to add.
//
// This component already subscribed to `sync.progress` and stored the payload
// in a reactive field, and `render()` never read it — the subscription was
// dead. It had nothing to show either: the only events were "started" and one
// terminal phase, so a sync that spent forty seconds fetching plugin files
// looked exactly like one that had hung. `detail` names the plugin and its
// position, which is the difference between slow and stuck.
function stepDetail(snap, progress) {
  if (!snap.sync_in_flight || !progress || !progress.detail) { return ""; }
  const terminal = ["completed", "cancelled", "failed"];
  if (terminal.includes(progress.phase)) { return ""; }
  return progress.detail;
}

export class SpSyncPill extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.progress = null;
    this.registerAction("cancel-sync", (_, e) => this._onCancel(e));
  }

  onConnect() {
    this.classList.add("sp-sync-pill");
    this.useSnapshot((s) => { this.snapshot = s; });
    this.bridgeSubscribe("sync.progress", (p) => {
      // A terminal phase clears the step rather than leaving the last plugin
      // name frozen beside "synced".
      this.progress = ["completed", "cancelled", "failed"].includes(p && p.phase) ? null : p;
    });
  }

  afterRender() {
    const snap = this.snapshot || {};
    const v = classify(snap);
    this.dataset.state = v.tone;
    this.title = snap.last_sync_summary
      ? (t("last-sync", { summary: snap.last_sync_summary }) || `Last sync: ${snap.last_sync_summary}`)
      : (t("last-sync-never") || "No syncs yet");
  }

  _onCancel(ev) {
    ev.preventDefault();
    ev.stopPropagation();
    runAction(ev.currentTarget, {
      run: () => bridge.cancel("sync"),
      context: t("sync-cancel") || "Cancel",
    });
  }

  render() {
    const snap = this.snapshot || {};
    const v = classify(snap);
    announce(v.text);
    const cancel = snap.sync_in_flight
      ? `<button type="button" class="sp-sync-pill__cancel" data-l10n-id="sync-cancel" data-l10n-aria="sync-cancel-aria" aria-label="Cancel sync" data-action="cancel-sync">Cancel</button>`
      : "";
    const detail = stepDetail(snap, this.progress);
    const step = detail
      ? `<span class="sp-sync-pill__step">${escapeHtml(detail)}</span>`
      : "";
    return `
      <span class="sp-sync-pill__dot" aria-hidden="true"></span>
      <span class="sp-sync-pill__label">${escapeHtml(v.text)}</span>
      ${step}
      ${cancel}
    `;
  }
}

reactive(SpSyncPill.prototype, ["snapshot", "progress"]);
customElements.define("sp-sync-pill", SpSyncPill);
