import { html, nothing } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

function classify(snap) {
  if (snap.sync_in_flight) {
    return { state: "running", text: t("sync-in-flight") || "syncing" };
  }
  if (snap.gateway_status && snap.gateway_status.state === "unreachable") {
    return { state: "err", text: t("gateway-unreachable") || "offline" };
  }
  if (snap.signed_in) {
    return { state: "ok", text: snap.last_sync_summary ? (t("sync-success") || "synced") : (t("ready") || "ready") };
  }
  return { state: "idle", text: t("gateway-not-signed-in") || "needs sign-in" };
}

export class SpSyncPill extends BridgeElement {
  static properties = { snapshot: { state: true }, progress: { state: true } };

  constructor() {
    super();
    this.snapshot = null;
    this.progress = null;
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    this.classList.add("sp-sync-pill");
    this.setAttribute("aria-live", "polite");
    bridge.stateSnapshot().then((s) => { this.snapshot = s; }).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => { this.snapshot = s; });
    this.bridgeSubscribe("sync.progress", (p) => { this.progress = p; });
  }

  updated() {
    const snap = this.snapshot || {};
    const v = classify(snap);
    this.dataset.state = v.state;
    this.title = snap.last_sync_summary
      ? (t("last-sync", { summary: snap.last_sync_summary }) || `Last sync: ${snap.last_sync_summary}`)
      : (t("last-sync-never") || "No syncs yet");
  }

  _onCancel(ev) {
    ev.preventDefault();
    ev.stopPropagation();
    bridge.cancel("sync").catch((e) => console.warn("cancel sync failed", e));
  }

  render() {
    const snap = this.snapshot || {};
    const v = classify(snap);
    return html`
      <span class="sp-sync-pill__dot" aria-hidden="true"></span>
      <span class="sp-sync-pill__label">${v.text}</span>
      ${snap.sync_in_flight
        ? html`<button type="button" class="sp-sync-pill__cancel" data-l10n-id="sync-cancel" aria-label="Cancel sync" @click=${(e) => this._onCancel(e)}>Cancel</button>`
        : nothing}
    `;
  }
}

customElements.define("sp-sync-pill", SpSyncPill);
