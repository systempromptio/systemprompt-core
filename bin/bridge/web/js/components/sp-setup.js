import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { bridge } from "/assets/js/bridge.js";
import { isInstalled } from "/assets/js/utils/verdict.js";
import { t } from "/assets/js/i18n.js";
import { notifyErr } from "/assets/js/utils/notify.js";
import "/assets/js/components/sp-setup-gateway.js";
import "/assets/js/components/sp-setup-agents.js";

const STEP_LABEL = {
  connect: () => t("setup-step-label-connect") || "Step 1 of 2",
  agents: () => t("setup-step-label-agents") || "Step 2 of 2",
};

// How long the wizard waits for every host to report before it says so. The
// gate had no timeout and no error path at all: one host that never reported
// left the user on onboarding indefinitely, with nothing on screen to say why.
const SETTLE_TIMEOUT_MS = 12_000;

function isConfigured(snap) {
  return !!snap.signed_in;
}

export class SpSetup extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.step = "connect";
    this.anyInstalled = false;
    this.firstRunActive = false;
    this.settleTimedOut = false;
    this.confirmEmptyFinish = false;
    this._settleTimer = null;
    this._finished = false;
    /** Latched once the app proper is on screen; see `_applySnapshot`. */
    this._leftSetup = false;
    this._logoFragment = null;
    this._onSetupOpen = () => { document.body.classList.add("is-setup-mode"); };
    this.registerAction("finish", () => this._finish());
    this.registerAction("finish-anyway", () => { this.confirmEmptyFinish = false; this._finish(true); });
    this.registerAction("retry-settle", () => {
      this.settleTimedOut = false;
      this._armSettleTimer();
      bridge.gatewayProbe().catch((e) => notifyErr(e, t("setup-retry") || "Check again"));
      for (const h of (this.snapshot && this.snapshot.host_apps) || []) {
        bridge.hostProbe(h.id).catch(() => {});
      }
    });
    this.registerAction("continue-anyway", () => {
      this._leftSetup = true;
      document.body.classList.remove("is-setup-mode");
    });
    this.registerAction("open-bridge", () => { this._leftSetup = true; document.body.classList.remove("is-setup-mode"); });
  }

  onConnect() {
    const tpl = this.querySelector('template[data-slot="logo"]');
    if (tpl) {
      this._logoFragment = tpl.content;
      tpl.remove();
    }
    bridge.stateSnapshot().then((s) => this._applySnapshot(s)).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => this._applySnapshot(s));
    this._unsubOpen = onBridgeEvent("setup-open", this._onSetupOpen);
  }

  onDisconnect() {
    if (this._unsubOpen) { this._unsubOpen(); this._unsubOpen = null; }
    this._clearSettleTimer();
  }

  _clearSettleTimer() {
    if (this._settleTimer) { clearTimeout(this._settleTimer); this._settleTimer = null; }
  }

  _armSettleTimer() {
    this._clearSettleTimer();
    this._settleTimer = setTimeout(() => {
      this._settleTimer = null;
      this.settleTimedOut = true;
    }, SETTLE_TIMEOUT_MS);
  }

  _applySnapshot(snap) {
    this.snapshot = snap;
    if (!snap) { return; }
    const configured = isConfigured(snap);
    const hosts = snap.host_apps || [];
    // Install state for a host is only KNOWN once its probe has completed, at
    // which point `snapshot` is populated. Until every host has a snapshot the
    // result is "unknown" — we must not show onboarding then, or it flashes
    // before detection resolves (the bug where it appeared with agents already
    // installed). Once settled, show the agents step only when none are
    // installed; installing one (anyInstalled) drops straight into the app.
    const settled = hosts.length > 0 && hosts.every((h) => h.snapshot);
    if (settled) {
      this._clearSettleTimer();
      this.settleTimedOut = false;
    } else if (!this._settleTimer && !this.settleTimedOut) {
      this._armSettleTimer();
    }
    const anyInstalled = hosts.some(isInstalled);
    this.anyInstalled = anyInstalled;
    this.step = configured ? "agents" : "connect";

    // First-use provisioning pins the wizard open. Checked before the
    // settled/latched guards below: a run is exactly the window in which host
    // snapshots are still arriving, so those guards would return early and let
    // the app show over a half-installed machine.
    this.firstRunActive = !!(snap.first_run && snap.first_run.active);
    if (this.firstRunActive) {
      this._leftSetup = false;
      document.body.classList.add("is-setup-mode");
      return;
    }

    // Signing out is the one thing that legitimately sends us back to the
    // splash. Clear the latch so it can.
    if (!snap.verified_identity || !snap.verified_identity.user_id) { this._leftSetup = false; }

    // Everything below decides whether to show a full-screen overlay, so it must
    // only ever run on a settled snapshot. `configured` and `anyInstalled` each
    // start out false and flip true as the gateway probe and then the host
    // probes land — evaluating on those partial snapshots is what made the
    // window flick splash → app → splash → app during startup.
    const gatewayProbing = !snap.gateway_status || !snap.gateway_status.settled;
    if (gatewayProbing || !settled) { return; }

    // One-way latch: once the app proper has been shown, a later probe result
    // must not yank the user back into onboarding mid-session.
    if (this._leftSetup) { return; }

    const needAgents = !anyInstalled && !this._finished;
    const inSetup = !configured || needAgents;
    if (!inSetup) { this._leftSetup = true; }
    document.body.classList.toggle("is-setup-mode", inSetup);
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
    this._leftSetup = true;
    document.body.classList.remove("is-setup-mode");
  }

  afterRender() {
    document.body.dataset.setupStep = this.step;
    const slot = this.querySelector("[data-logo-slot]");
    if (slot && this._logoFragment && !slot.firstElementChild) {
      slot.append(this._logoFragment.cloneNode(true));
    }
  }

  _settleNotice() {
    if (!this.settleTimedOut) { return ""; }
    const snap = this.snapshot || {};
    const unreachable = snap.gateway_status && snap.gateway_status.tone === "err";
    const line = unreachable
      ? t("setup-settle-unreachable", { gateway: snap.gateway_url || "" })
      : t("setup-settle-slow") || "Still checking this computer. Some agents have not reported yet.";
    return `
      <div class="sp-setup__note sp-setup__note--warn" role="alert">
        <p>${escapeHtml(line)}</p>
        <div class="sp-setup__actions">
          <button class="sp-btn-ghost" type="button" data-action="retry-settle">${escapeHtml(t("setup-retry") || "Check again")}</button>
          <button class="sp-btn-ghost" type="button" data-action="continue-anyway">${escapeHtml(t("setup-continue-anyway") || "Continue anyway")}</button>
        </div>
      </div>
    `;
  }

  render() {
    const step = this.step;
    const stepLabel = (STEP_LABEL[step] || (() => ""))();
    const version = this.dataset.version || "";
    const platform = this.dataset.platform || "linux";
    const platformDisplay = this.dataset.platformDisplay || "";
    // Finish is enabled except while first-use provisioning is running. Host
    // install-state is probe-driven and can lag or misreport (e.g. the card
    // shows "Installed ✓" while `anyInstalled` is still false), which trapped
    // the user on this step with no way forward — so it is never gated on that.
    // An in-flight run is different: it is a bounded operation that reports its
    // own completion, and leaving mid-run is what produced a broken app.
    const finishDisabled = this.firstRunActive ? "disabled" : "";
    return `
      <div class="sp-setup__card">
        <div class="sp-setup__hero">
          <div class="sp-setup__mark" data-logo-slot data-preserve></div>
          <div class="sp-setup__eyebrow"><span data-l10n-id="setup-eyebrow-prefix">DEMO BUILD</span> · v${escapeHtml(version)} · <span>${escapeHtml(stepLabel)}</span></div>
          <h1 data-l10n-id="setup-heading">Welcome to systemprompt bridge</h1>
          <p class="sp-setup__lede" data-l10n-id="setup-lede">systemprompt bridge routes one or more coding agents through your enterprise gateway.</p>
        </div>
        <div class="sp-setup__step" data-step="connect" ${step !== "connect" ? "hidden" : ""}>
          <sp-setup-gateway></sp-setup-gateway>
        </div>
        <div class="sp-setup__step" data-step="agents" ${step !== "agents" ? "hidden" : ""}>
          <p class="sp-setup__lede" data-l10n-id="setup-agents-lede">Pick the coding agents you want systemprompt bridge to govern.</p>
          <sp-setup-agents></sp-setup-agents>
          ${this.confirmEmptyFinish
            ? `<p class="sp-setup__note" role="alert">${escapeHtml(t("setup-finish-empty-warning") || "You have not added an agent yet, so nothing will be routed through systemprompt.")}</p>`
            : ""}
          <div class="sp-setup__actions">
            ${this.confirmEmptyFinish
              ? `<button class="sp-btn-primary" type="button" data-action="finish-anyway">${escapeHtml(t("setup-finish-anyway") || "Finish anyway")}</button>`
              : `<button class="sp-btn-primary" type="button" data-l10n-id="setup-finish" data-action="finish" ${finishDisabled}>Finish</button>`}
          </div>
        </div>
        ${this._settleNotice()}
        <aside class="sp-setup__warning" role="note">
          <strong data-l10n-id="setup-warning-strong">Demo software.</strong>
          <span data-l10n-id="setup-warning-body">This build is provided for demonstration purposes only and is not licensed for production use.</span>
        </aside>
        <p class="sp-setup__meta">
          <a class="sp-setup__docs" href="https://systemprompt.io/docs/bridge/${escapeHtml(platform)}" target="_blank" rel="noopener noreferrer">
            Documentation for ${escapeHtml(platformDisplay)} →
          </a>
          <span class="sp-setup__meta-sep">·</span>
          <span>Licensing — <a href="mailto:ed@systemprompt.io?subject=systemprompt%20bridge%20licensing">ed@systemprompt.io</a></span>
        </p>
      </div>
    `;
  }
}

reactive(SpSetup.prototype, ["snapshot", "step", "anyInstalled", "firstRunActive", "settleTimedOut", "confirmEmptyFinish"]);
customElements.define("sp-setup", SpSetup);
