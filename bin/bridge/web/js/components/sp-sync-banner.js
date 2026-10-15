import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { runAction } from "/assets/js/utils/action.js";

/**
 * A failed sync, said in plain words with a Retry button, on the tab where its
 * consequences show (agents without their skills). It used to reach only the
 * activity log, which nobody reads while wondering why an agent is empty.
 *
 * Driven by `sync.progress`: a `failed` event carries the reason the bridge
 * already localised; any later event means a new run, which clears it.
 */
export class SpSyncBanner extends SpElement {
  constructor() {
    super();
    this.failure = null;
    this.registerAction("retry-sync", (trigger) => runAction(trigger, {
      run: () => bridge.sync(),
      context: t("sync-retry") || "Retry sync",
    }));
  }

  onConnect() {
    this.bridgeSubscribe("sync.progress", (p) => {
      if (!p) { return; }
      this.failure = p.failed === true ? (p.summary || t("sync-failed-banner-generic") || "") : null;
    });
  }

  render() {
    if (this.failure === null) { return ""; }
    return `<div class="sp-agents-banner" role="alert" data-tone="err">
      <div class="sp-agents-banner__text">
        <span class="sp-agents-banner__title">${escapeHtml(t("sync-failed-banner-title") || "Sync failed")}</span>
        <span class="sp-agents-banner__detail">${escapeHtml(this.failure)}</span>
      </div>
      <button class="sp-btn-primary" type="button" data-action="retry-sync">${escapeHtml(t("sync-retry") || "Retry sync")}</button>
    </div>`;
  }
}

reactive(SpSyncBanner.prototype, ["failure"]);
customElements.define("sp-sync-banner", SpSyncBanner);
