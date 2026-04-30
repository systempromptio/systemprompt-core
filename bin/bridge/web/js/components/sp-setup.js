import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-setup-gateway.js";
import "/assets/js/components/sp-setup-agents.js";

const STEP_LABEL = {
  connect: () => t("setup-step-label-connect") || "Step 1 of 3",
  agents: () => t("setup-step-label-agents") || "Step 2 of 3",
  done: () => t("setup-step-label-done") || "Step 3 of 3",
};

function isConfigured(snap) {
  const reachable = snap.gateway_status && snap.gateway_status.state === "reachable";
  const id = snap.verified_identity;
  return !!(reachable && id && id.user_id);
}

export class SpSetup extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.step = "connect";
    this._logoFragment = null;
    this._onSetupOpen = () => { document.body.classList.add("is-setup-mode"); };
    this.registerAction("complete", () => this._complete());
    this.registerAction("open-bridge", () => { document.body.classList.remove("is-setup-mode"); });
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
  }

  _applySnapshot(snap) {
    this.snapshot = snap;
    if (!snap) { return; }
    const configured = isConfigured(snap);
    const onboarded = snap.agents_onboarded === true;
    const anyInstalled = (snap.host_apps || []).some((h) => h.snapshot?.profile_state?.kind === "installed");
    const inSetup = !(configured && (onboarded || anyInstalled));
    document.body.classList.toggle("is-setup-mode", inSetup);
    this.step = configured ? "agents" : "connect";
  }

  async _complete() {
    try { await bridge.setupComplete(); } catch (err) { console.warn("setup complete", err); }
    this.step = "done";
  }

  afterRender() {
    document.body.dataset.setupStep = this.step;
    const slot = this.querySelector("[data-logo-slot]");
    if (slot && this._logoFragment && !slot.firstElementChild) {
      slot.append(this._logoFragment.cloneNode(true));
    }
  }

  render() {
    const step = this.step;
    const stepLabel = (STEP_LABEL[step] || (() => ""))();
    const version = this.dataset.version || "";
    const platform = this.dataset.platform || "linux";
    const platformDisplay = this.dataset.platformDisplay || "";
    return `
      <div class="sp-setup__card">
        <div class="sp-setup__hero">
          <div class="sp-setup__mark" data-logo-slot></div>
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
          <div class="sp-setup__actions">
            <button class="sp-btn-ghost" type="button" data-l10n-id="setup-skip-agents" data-action="complete">Skip — set up later</button>
            <button class="sp-btn-primary" type="button" data-l10n-id="setup-finish" data-action="complete">Finish</button>
          </div>
        </div>
        <div class="sp-setup__step" data-step="done" ${step !== "done" ? "hidden" : ""}>
          <p class="sp-setup__lede" data-l10n-id="setup-done-lede">systemprompt bridge is ready.</p>
          <div class="sp-setup__actions">
            <button class="sp-btn-primary" type="button" data-l10n-id="setup-open" data-action="open-bridge">Open systemprompt bridge</button>
          </div>
        </div>
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

reactive(SpSetup.prototype, ["snapshot", "step"]);
customElements.define("sp-setup", SpSetup);
