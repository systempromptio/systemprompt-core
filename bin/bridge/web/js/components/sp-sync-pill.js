import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { announce } from "/assets/js/utils/announce.js";
import { runAction } from "/assets/js/utils/action.js";

// `overall` is the bridge's one-dot summary; the footer draws the same one.
function classify(snap) {
  const overall = snap.overall || { tone: "unknown", code: "needs-sign-in" };
  return { tone: overall.tone, text: t(`overall-${overall.code}`) || "" };
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
    this.bridgeSubscribe("sync.progress", (p) => { this.progress = p; });
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
    return `
      <span class="sp-sync-pill__dot" aria-hidden="true"></span>
      <span class="sp-sync-pill__label">${escapeHtml(v.text)}</span>
      ${cancel}
    `;
  }
}

reactive(SpSyncPill.prototype, ["snapshot", "progress"]);
customElements.define("sp-sync-pill", SpSyncPill);
