import { SpElement, escapeHtml } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";
import { probeView, probeErrorMessage, isPendingResolved, patLinkFor } from "/assets/js/utils/gateway.js";

const PERSIST_DEBOUNCE_MS = 600;

export class SpSetupGateway extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.gateway = "";
    this.pat = "";
    this.patSaved = false;
    this.error = "";
    this.pending = false;
    this._lastSavedGateway = "";
    this._debounce = null;
    this._pendingSince = 0;
    this._initialRendered = false;
    this.registerAction("connect", () => this._connect());
    this.registerAction("edit-pat", () => this._editPat());
    this.registerAction("input:gateway", (trigger) => this._onGatewayInput(trigger));
    this.registerAction("input:pat", (trigger) => { this.pat = trigger.value; });
    this.addEventListener("focusin", (e) => {
      if (e.target.id === "setup-pat" && this.patSaved) {
        this.pat = ""; this.patSaved = false; this._syncInputs();
      }
    });
    this.addEventListener("blur", (e) => {
      if (e.target && e.target.id === "setup-gateway") { this._onGatewayBlur(); }
    }, true);
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => this._applySnapshot(s)).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => this._applySnapshot(s));
  }

  onDisconnect() {
    if (this._debounce) { clearTimeout(this._debounce); }
  }

  _applySnapshot(snap) {
    this.snapshot = snap;
    if (snap && document.activeElement && document.activeElement.id !== "setup-gateway") {
      if (snap.gateway_url && this.gateway !== snap.gateway_url) {
        this.gateway = snap.gateway_url;
        this._lastSavedGateway = snap.gateway_url;
      }
    }
    if (snap) {
      if (snap.pat_present && !this.patSaved && !this.pat) {
        this.pat = "•".repeat(24);
        this.patSaved = true;
      } else if (!snap.pat_present && this.patSaved) {
        this.pat = ""; this.patSaved = false;
      }
    }
    if (this.pending && isPendingResolved(snap, this._pendingSince)) {
      this.pending = false; this._pendingSince = 0;
    }
    const newError = probeErrorMessage(snap);
    if (newError) { this.error = newError; }
    else if (this.error && !this.pending) { this.error = ""; }
    this._render();
  }

  _onGatewayInput(input) {
    this.gateway = input.value;
    if (this._debounce) { clearTimeout(this._debounce); }
    this._debounce = setTimeout(() => this._persistGateway(), PERSIST_DEBOUNCE_MS);
  }

  _onGatewayBlur() {
    if (this._debounce) { clearTimeout(this._debounce); }
    this._persistGateway();
  }

  async _persistGateway() {
    const url = (this.gateway || "").trim();
    if (url && url !== this._lastSavedGateway) {
      this._lastSavedGateway = url;
      try { await bridge.gatewaySet(url); } catch (e) { console.warn("gateway set", e); }
    }
  }

  _editPat() {
    if (this.patSaved) { this.pat = ""; this.patSaved = false; this._render(); }
    setTimeout(() => {
      const input = this.querySelector("#setup-pat");
      if (input) { input.focus(); }
    }, 0);
  }

  async _connect() {
    const gw = (this.gateway || "").trim();
    if (!gw) { this.error = "Enter the gateway URL."; this._render(); return; }
    if (!/^https?:\/\//i.test(gw)) { this.error = "Gateway URL must start with http:// or https://"; this._render(); return; }
    this._lastSavedGateway = gw;
    this.pending = true; this._pendingSince = Date.now();
    this.error = "";
    this._render();
    try {
      if (this.patSaved) {
        await bridge.gatewayProbe();
      } else {
        const token = (this.pat || "").trim();
        if (!token) { this.error = "Paste your personal access token."; this.pending = false; this._pendingSince = 0; this._render(); return; }
        await bridge.login(token, gw);
      }
    } catch (err) {
      this.error = `${this.patSaved ? "Probe" : "Login"} failed: ${(err && err.message) || err}`;
      this.pending = false; this._pendingSince = 0;
      this._render();
    }
  }

  _render() { this.invalidate(); }

  afterRender() { this._syncInputs(); }

  _syncInputs() {
    const gw = this.querySelector("#setup-gateway");
    if (gw && document.activeElement !== gw && gw.value !== this.gateway) { gw.value = this.gateway; }
    const pat = this.querySelector("#setup-pat");
    if (pat && document.activeElement !== pat && pat.value !== this.pat) { pat.value = this.pat; }
  }

  render() {
    const probe = probeView(this.snapshot);
    const link = patLinkFor(this.gateway);
    const linkDisabled = link === "#";
    const editBtn = this.patSaved ? `<button class="sp-btn-ghost" type="button" data-action="edit-pat">Edit</button>` : "";
    const errBlock = this.error ? `<span class="sp-setup__error">${escapeHtml(this.error)}</span>` : "";
    const btnLabel = this.pending ? (t("setup-connecting") || "Connecting…") : "Connect";
    return `
      <div class="sp-setup__field">
        <label for="setup-gateway" data-l10n-id="setup-gateway-label">Gateway URL</label>
        <input id="setup-gateway" type="url" placeholder="http://127.0.0.1:8080" autocomplete="off" spellcheck="false" data-input="gateway" />
        <div class="sp-setup__status">
          <span class="sp-dot ${probe.dot}" aria-hidden="true"></span>
          <span class="${probe.muted ? "sp-u-muted" : ""}">${escapeHtml(probe.text)}</span>
        </div>
      </div>
      <div class="sp-setup__field">
        <label for="setup-pat" data-l10n-id="setup-pat-label">Personal access token</label>
        <input id="setup-pat" type="password" placeholder="sp-live-…" autocomplete="off" spellcheck="false" data-input="pat" />
        <p class="sp-setup__hint">
          <span data-l10n-id="setup-pat-hint">Don't have one yet?</span>
          <a class="sp-setup__pat-link ${linkDisabled ? "is-disabled" : ""}" href="${escapeHtml(link)}" target="_blank" rel="noopener noreferrer" aria-disabled="${linkDisabled}">Open the gateway admin login →</a>
          ${editBtn}
        </p>
      </div>
      <div class="sp-setup__actions">
        <button class="sp-btn-primary" type="button" ${this.pending ? "disabled" : ""} data-action="connect">
          <span class="sp-btn__label">${escapeHtml(btnLabel)}</span>
        </button>
        ${errBlock}
      </div>
    `;
  }
}

customElements.define("sp-setup-gateway", SpSetupGateway);
