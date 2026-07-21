import { SpElement, reactive, escapeHtml } from "/assets/js/components/sp-element.js";
import { onBridgeEvent } from "/assets/js/events/bridge-events.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import "/assets/js/components/sp-setup-gateway.js";
import "/assets/js/components/sp-setup-agents.js";

const STEP_LABEL = {
  connect: () => t("setup-step-label-connect") || "Step 1 of 2",
  agents: () => t("setup-step-label-agents") || "Step 2 of 2",
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
    this.anyInstalled = false;
    this._finished = false;
    /** Latched once the app proper is on screen; see `_applySnapshot`. */
    this._leftSetup = false;
    this._logoFragment = null;
    this._onSetupOpen = () => { document.body.classList.add("is-setup-mode"); };
    this.registerAction("finish", () => this._finish());
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
    const anyInstalled = hosts.some((h) => h.snapshot?.profile_state?.kind === "installed");
    this.anyInstalled = anyInstalled;
    this.step = configured ? "agents" : "connect";

    // Signing out is the one thing that legitimately sends us back to the
    // splash. Clear the latch so it can.
    if (!snap.verified_identity || !snap.verified_identity.user_id) { this._leftSetup = false; }

    // Everything below decides whether to show a full-screen overlay, so it must
    // only ever run on a settled snapshot. `configured` and `anyInstalled` each
    // start out false and flip true as the gateway probe and then the host
    // probes land — evaluating on those partial snapshots is what made the
    // window flick splash → app → splash → app during startup.
    const gatewayProbing = !snap.gateway_status || snap.gateway_status.state === "probing"
      || snap.gateway_status.state === "unknown";
    if (gatewayProbing || !settled) { return; }

    // One-way latch: once the app proper has been shown, a later probe result
    // must not yank the user back into onboarding mid-session.
    if (this._leftSetup) { return; }

    const needAgents = !anyInstalled && !this._finished;
    const inSetup = !configured || needAgents;
    if (!inSetup) { this._leftSetup = true; }
    document.body.classList.toggle("is-setup-mode", inSetup);
  }

  _finish() {
    this._finished = true;
    this._leftSetup = true;
    bridge.setupComplete().catch((err) => console.warn("setup complete", err));
    document.body.classList.remove("is-setup-mode");
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
    // Finish is always enabled. Host install-state is probe-driven and can lag
    // or misreport (e.g. the card shows "Installed ✓" while `anyInstalled` is
    // still false), which trapped the user on this step with no way forward.
    // Installing agents is optional, so never block completing setup.
    const finishDisabled = "";
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
            <button class="sp-btn-primary" type="button" data-l10n-id="setup-finish" data-action="finish" ${finishDisabled}>Finish</button>
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

reactive(SpSetup.prototype, ["snapshot", "step", "anyInstalled"]);
customElements.define("sp-setup", SpSetup);
