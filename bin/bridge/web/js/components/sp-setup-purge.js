import { SpElement } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { escapeHtml } from "/assets/js/utils/escape.js";
import { notifyErr } from "/assets/js/utils/notify.js";

// Folded away under the sign-in form: a full purge is a recovery tool, not a
// step of setup, so it earns a disclosure and a second click, nothing more.
export class SpSetupPurge extends SpElement {
  constructor() {
    super();
    this.confirming = false;
    this.busy = false;
    this.registerAction("purge-ask", () => { this.confirming = true; this.invalidate(); });
    this.registerAction("purge-cancel", () => { this.confirming = false; this.invalidate(); });
    this.registerAction("purge-confirm", () => this._purge());
  }

  async _purge() {
    if (this.busy) { return; }
    this.busy = true;
    this.invalidate();
    try {
      await bridge.systemPurge();
      this.confirming = false;
    } catch (err) {
      notifyErr(err, t("toast-purge-failed") || "Could not remove everything");
    } finally {
      this.busy = false;
      this.invalidate();
    }
  }

  render() {
    const body = this.confirming
      ? `
        <p class="sp-setup__hint">${escapeHtml(t("setup-purge-confirm") || "Remove everything the bridge installed on this computer? This cannot be undone.")}</p>
        <div class="sp-setup__actions">
          <button class="sp-btn-danger" type="button" data-action="purge-confirm" ${this.busy ? "disabled" : ""}>
            <span class="sp-btn__label">${escapeHtml(this.busy ? (t("setup-purge-working") || "Removing…") : (t("setup-purge-confirm-button") || "Yes, remove it all"))}</span>
          </button>
          <button class="sp-btn-ghost" type="button" data-action="purge-cancel" ${this.busy ? "disabled" : ""}>
            <span class="sp-btn__label">${escapeHtml(t("setup-purge-cancel") || "Keep it")}</span>
          </button>
        </div>`
      : `
        <p class="sp-setup__hint">${escapeHtml(t("setup-purge-explainer") || "Removes the bridge's installed plugins, scheduled sync, managed profile, sign-in, identity and every saved setting. The app returns to this screen as if it had never been set up.")}</p>
        <div class="sp-setup__actions">
          <button class="sp-btn-ghost sp-btn-ghost--danger" type="button" data-action="purge-ask">
            <span class="sp-btn__label">${escapeHtml(t("setup-purge-button") || "Remove everything")}</span>
          </button>
        </div>`;
    return `
      <details class="sp-setup__advanced sp-setup__advanced--danger">
        <summary data-l10n-id="setup-purge-summary">Remove everything from this computer</summary>
        ${body}
      </details>
    `;
  }
}

customElements.define("sp-setup-purge", SpSetupPurge);
