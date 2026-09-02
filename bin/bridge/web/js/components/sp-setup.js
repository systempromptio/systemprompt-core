import { SpElement, reactive } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { notifyErr } from "/assets/js/utils/notify.js";
import { stepsFromSnapshot } from "/assets/js/utils/setup-steps.js";
import { clearSettleTimer, trackSettle, retrySettle } from "/assets/js/utils/setup-settle.js";
import { renderSetupBrand, renderSetupHeading, renderSetupFinalizing, renderSetupAgentsStep, renderSetupSettleNotice } from "/assets/js/components/setup-sections.js";
import "/assets/js/components/sp-setup-gateway.js";
import "/assets/js/components/sp-setup-agents.js";

export class SpSetup extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.step = "connect";
    this.anyInstalled = false;
    this.finalizing = false;
    this.firstRunActive = false;
    this.settleTimedOut = false;
    this.confirmEmptyFinish = false;
    this._settleTimer = null;
    this._finished = false;
    /** Latched once the app proper is on screen; see `stepsFromSnapshot`. */
    this._leftSetup = false;
    this._logoFragment = null;
    this._onSetupOpen = () => { document.body.classList.add("is-setup-mode"); };
    this.registerAction("finish", () => this._finish());
    this.registerAction("finish-anyway", () => { this.confirmEmptyFinish = false; this._finish(true); });
    this.registerAction("retry-settle", () => retrySettle(this));
    this.registerAction("continue-anyway", () => this._leaveSetup());
    this.registerAction("open-bridge", () => this._leaveSetup());
  }

  onConnect() {
    const tpl = this.querySelector('template[data-slot="logo"]');
    if (tpl) {
      this._logoFragment = tpl.content;
      tpl.remove();
    }
    this.useSnapshot((s) => this._applySnapshot(s));
    this._unsubOpen = onBridgeEvent("setup-open", this._onSetupOpen);
  }

  onDisconnect() {
    if (this._unsubOpen) { this._unsubOpen(); this._unsubOpen = null; }
    clearSettleTimer(this);
  }

  _leaveSetup() {
    this._leftSetup = true;
    document.body.classList.remove("is-setup-mode");
  }

  _applySnapshot(snap) {
    this.snapshot = snap;
    if (!snap) { return; }
    const model = stepsFromSnapshot(snap, { leftSetup: this._leftSetup, finished: this._finished });
    trackSettle(this, model.settled);
    this.anyInstalled = model.anyInstalled;
    this.step = model.step;
    this.finalizing = model.finalizing;
    this.firstRunActive = model.firstRunActive;
    this._leftSetup = model.leftSetup;
    if (model.setupMode !== null) { document.body.classList.toggle("is-setup-mode", model.setupMode); }
  }

  async _finish(force = false) {
    if (this.firstRunActive) { return; }
    // Finish stays enabled — gating it on `anyInstalled` trapped users whose
    // per-host label disagreed with it — but pressing it having added nothing
    // is worth one question.
    if (!this.anyInstalled && !force) { this.confirmEmptyFinish = true; return; }
    try {
      await bridge.setupComplete();
    } catch (err) {
      // Dismissing the wizard on a failed call dropped the user into an app
      // whose setup was never recorded, and put them back through onboarding
      // on the next launch.
      notifyErr(err, t("toast-setup-complete-failed") || "Could not record that setup finished.");
      return;
    }
    this._finished = true;
    this._leaveSetup();
  }

  afterRender() {
    document.body.dataset.setupStep = this.step;
    const slot = this.querySelector("[data-logo-slot]");
    if (slot && this._logoFragment && !slot.firstElementChild) {
      slot.append(this._logoFragment.cloneNode(true));
    }
  }

  render() {
    return `
      <div class="sp-setup__split">
        ${renderSetupBrand(this)}
        <section class="sp-setup__panel">
          <div class="sp-setup__panel-inner">
            ${this.finalizing && !this.settleTimedOut
              ? renderSetupFinalizing(this)
              : `
                ${renderSetupHeading(this)}
                <div class="sp-setup__step" data-step="connect" ${this.step !== "connect" ? "hidden" : ""}>
                  <sp-setup-gateway></sp-setup-gateway>
                </div>
                ${renderSetupAgentsStep(this)}
                ${renderSetupSettleNotice(this)}
              `}
          </div>
        </section>
      </div>
    `;
  }
}

reactive(SpSetup.prototype, ["snapshot", "step", "anyInstalled", "finalizing", "firstRunActive", "settleTimedOut", "confirmEmptyFinish"]);
customElements.define("sp-setup", SpSetup);
