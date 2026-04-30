import { SpElement } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { probeErrorMessage, isPendingResolved, renderGatewayForm } from "/assets/js/utils/gateway.js";

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
    this.registerAction("connect", () => this._connect());
    this.registerAction("edit-pat", () => this._editPat());
    this.registerAction("input:gateway", (t) => this._onGatewayInput(t));
    this.registerAction("input:pat", (t) => { this.pat = t.value; });
    this.addEventListener("focusin", (e) => {
      if (e.target.id === "setup-pat" && this.patSaved) {
        this.pat = ""; this.patSaved = false; this._syncInputs();
      }
    });
    this.addEventListener("blur", (e) => {
      if (e.target && e.target.id === "setup-gateway") {
        if (this._debounce) { clearTimeout(this._debounce); }
        this._persistGateway();
      }
    }, true);
  }

  onConnect() {
    bridge.stateSnapshot().then((s) => this._applySnapshot(s)).catch((e) => console.warn("snapshot failed", e));
    this.bridgeSubscribe("state.changed", (s) => this._applySnapshot(s));
  }

  onDisconnect() { if (this._debounce) { clearTimeout(this._debounce); } }

  _applySnapshot(snap) {
    this.snapshot = snap;
    if (!snap) { return; }
    if (document.activeElement && document.activeElement.id !== "setup-gateway"
        && snap.gateway_url && this.gateway !== snap.gateway_url) {
      this.gateway = snap.gateway_url;
      this._lastSavedGateway = snap.gateway_url;
    }
    if (snap.pat_present && !this.patSaved && !this.pat) {
      this.pat = "•".repeat(24); this.patSaved = true;
    } else if (!snap.pat_present && this.patSaved) {
      this.pat = ""; this.patSaved = false;
    }
    if (this.pending && isPendingResolved(snap, this._pendingSince)) {
      this.pending = false; this._pendingSince = 0;
    }
    const newError = probeErrorMessage(snap);
    if (newError) { this.error = newError; }
    else if (this.error && !this.pending) { this.error = ""; }
    this.invalidate();
  }

  _onGatewayInput(input) {
    this.gateway = input.value;
    if (this._debounce) { clearTimeout(this._debounce); }
    this._debounce = setTimeout(() => this._persistGateway(), PERSIST_DEBOUNCE_MS);
  }

  async _persistGateway() {
    const url = (this.gateway || "").trim();
    if (url && url !== this._lastSavedGateway) {
      this._lastSavedGateway = url;
      try { await bridge.gatewaySet(url); } catch (e) { console.warn("gateway set", e); }
    }
  }

  _editPat() {
    if (this.patSaved) { this.pat = ""; this.patSaved = false; this.invalidate(); }
    setTimeout(() => {
      const input = this.querySelector("#setup-pat");
      if (input) { input.focus(); }
    }, 0);
  }

  async _connect() {
    const gw = (this.gateway || "").trim();
    if (!gw) { this.error = "Enter the gateway URL."; this.invalidate(); return; }
    if (!/^https?:\/\//i.test(gw)) { this.error = "Gateway URL must start with http:// or https://"; this.invalidate(); return; }
    this._lastSavedGateway = gw;
    this.pending = true; this._pendingSince = Date.now(); this.error = ""; this.invalidate();
    try {
      if (this.patSaved) { await bridge.gatewayProbe(); }
      else {
        const token = (this.pat || "").trim();
        if (!token) { this.error = "Paste your personal access token."; this.pending = false; this._pendingSince = 0; this.invalidate(); return; }
        await bridge.login(token, gw);
      }
    } catch (err) {
      this.error = `${this.patSaved ? "Probe" : "Login"} failed: ${(err && err.message) || err}`;
      this.pending = false; this._pendingSince = 0; this.invalidate();
    }
  }

  afterRender() { this._syncInputs(); }

  _syncInputs() {
    const gw = this.querySelector("#setup-gateway");
    if (gw && document.activeElement !== gw && gw.value !== this.gateway) { gw.value = this.gateway; }
    const pat = this.querySelector("#setup-pat");
    if (pat && document.activeElement !== pat && pat.value !== this.pat) { pat.value = this.pat; }
  }

  render() { return renderGatewayForm(this); }
}

customElements.define("sp-setup-gateway", SpSetupGateway);
