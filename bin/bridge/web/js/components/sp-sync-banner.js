import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { runAction } from "/assets/js/utils/action.js";

/**
 * A sync that did not finish, said in plain words with a Retry button, on the tab where its
 * consequences show (agents without their skills). It used to reach only the
 * activity log, which nobody reads while wondering why an agent is empty.
 *
 * Driven by `sync.progress`: a `failed` event carries the reason the bridge
 * already localised; any later event means a new run, which clears it.
 * `gateway.trust.required` replaces the retry with the signing-key review: the
 * fingerprint is shown and the key is pinned only when the user confirms it.
 */
export class SpSyncBanner extends SpElement {
  constructor() {
    super();
    this.failure = null;
    this.trust = null;
    this.registerAction("retry-sync", (trigger) => runAction(trigger, {
      run: () => bridge.sync(),
      context: t("sync-retry") || "Retry sync",
    }));
    this.registerAction("trust-review", (trigger) => this._reviewKey(trigger));
    this.registerAction("trust-confirm", (trigger) => this._confirmKey(trigger));
    this.registerAction("trust-cancel", () => { this.trust = { stage: "needed" }; });
  }

  onConnect() {
    this.bridgeSubscribe("sync.progress", (p) => {
      if (!p) { return; }
      this.failure = p.failed === true ? (p.summary || t("sync-failed-banner-generic") || "") : null;
      if (p.failed !== true) { this.trust = null; }
    });
    this.bridgeSubscribe("gateway.trust.required", () => { this.trust = { stage: "needed" }; });
  }

  async _reviewKey(trigger) {
    const result = await runAction(trigger, {
      run: () => bridge.gatewayTrustInspect(),
      context: t("trust-review") || "Review signing key",
    });
    if (result.ok) {
      this.trust = { stage: "confirm", gateway: result.value.gateway, fingerprint: result.value.fingerprint };
    }
  }

  async _confirmKey(trigger) {
    const pending = this.trust;
    if (!pending || pending.stage !== "confirm") { return; }
    const result = await runAction(trigger, {
      run: () => bridge.gatewayTrustConfirm(pending.gateway, pending.fingerprint),
      success: t("trust-confirmed") || "Signing key trusted. Syncing now.",
      context: t("trust-confirm") || "Trust this key",
    });
    if (result.ok) {
      this.trust = null;
      this.failure = null;
    }
  }

  _renderTrust() {
    if (this.trust.stage === "confirm") {
      const grouped = String(this.trust.fingerprint).match(/.{1,4}/g).join(" ");
      return `<div class="sp-agents-banner" role="alertdialog" aria-labelledby="sp-trust-title">
      <div class="sp-agents-banner__text">
        <span class="sp-agents-banner__title" id="sp-trust-title">${escapeHtml(t("trust-confirm-title") || "Is this your organisation's signing key?")}</span>
        <span class="sp-agents-banner__detail">${escapeHtml(t("trust-confirm-detail") || "Check this fingerprint with your administrator before you trust it. Everything this gateway sends will be checked against it from now on.")}</span>
        <span class="sp-agents-banner__detail">${escapeHtml(this.trust.gateway)}</span>
        <code class="sp-agents-banner__detail">${escapeHtml(grouped)}</code>
      </div>
      <button class="sp-btn-primary" type="button" data-action="trust-confirm">${escapeHtml(t("trust-confirm") || "Trust this key")}</button>
      <button class="sp-btn-ghost" type="button" data-action="trust-cancel">${escapeHtml(t("trust-cancel") || "Not now")}</button>
    </div>`;
    }
    return `<div class="sp-agents-banner" role="status">
      <div class="sp-agents-banner__text">
        <span class="sp-agents-banner__title">${escapeHtml(t("trust-needed-title") || "This gateway's signing key isn't trusted yet")}</span>
        <span class="sp-agents-banner__detail">${escapeHtml(t("trust-needed-detail") || "Nothing syncs until you confirm the key's fingerprint.")}</span>
      </div>
      <button class="sp-btn-primary" type="button" data-action="trust-review">${escapeHtml(t("trust-review") || "Review signing key")}</button>
    </div>`;
  }

  render() {
    if (this.trust !== null) { return this._renderTrust(); }
    if (this.failure === null) { return ""; }
    return `<div class="sp-agents-banner" role="status">
      <div class="sp-agents-banner__text">
        <span class="sp-agents-banner__title">${escapeHtml(t("sync-failed-banner-title") || "Not everything synced")}</span>
        <span class="sp-agents-banner__detail">${escapeHtml(this.failure)}</span>
      </div>
      <button class="sp-btn-primary" type="button" data-action="retry-sync">${escapeHtml(t("sync-retry") || "Retry sync")}</button>
    </div>`;
  }
}

reactive(SpSyncBanner.prototype, ["failure", "trust"]);
customElements.define("sp-sync-banner", SpSyncBanner);
