import { SpElement } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { probeErrorMessage, isPendingResolved, renderGatewayForm } from "/assets/js/utils/gateway.js";
import {
  persistGatewayUrl, signInToGateway, cancelGatewaySignIn, connectGatewayWithPat,
  clearGatewayPendingTimer, resolveGatewayPending,
} from "/assets/js/utils/gateway-signin.js";

export class SpSetupGateway extends SpElement {
  constructor() {
    super();
    this.snapshot = null;
    this.gateway = "";
    this.pat = "";
    this.patSaved = false;
    this.error = "";
    this.pending = false;
    this.signingIn = false;
    this.keepSignedIn = true;
    this._lastSavedGateway = "";
    this._pendingSince = 0;
    this._pendingTimer = null;
    this.registerAction("sign-in", () => signInToGateway(this));
    this.registerAction("cancel-sign-in", () => cancelGatewaySignIn(this));
    this.registerAction("connect", () => connectGatewayWithPat(this));
    this.registerAction("edit-pat", () => this._editPat());
    this.registerAction("input:gateway", (input) => { this.gateway = input.value; });
    this.registerAction("input:pat", (input) => { this.pat = input.value; });
    this.registerAction("input:keep", (input) => { this.keepSignedIn = !!input.checked; });
    this._bindFieldEvents();
  }

  _bindFieldEvents() {
    this.addEventListener("focusin", (e) => {
      if (e.target.id === "setup-pat" && this.patSaved) {
        this.pat = ""; this.patSaved = false; this._syncInputs();
      }
    });
    // The URL is committed when the field is left or an action is pressed, not
    // on every keystroke: a 600ms debounce was writing half-typed URLs to disk.
    this.addEventListener("blur", (e) => {
      if (e.target && e.target.id === "setup-gateway") { persistGatewayUrl(this); }
    }, true);
  }

  onConnect() {
    this.useSnapshot((s) => this._applySnapshot(s));
    // Sign-in is gated on a reachable gateway, so the probe has to run before
    // first paint — otherwise the button sits disabled on a stale "unknown".
    bridge.gatewayProbe().catch((e) => console.warn("initial probe", e));
  }

  onDisconnect() {
    clearGatewayPendingTimer(this);
  }

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
      resolveGatewayPending(this);
    }
    const newError = probeErrorMessage(snap);
    if (newError) { this.error = newError; }
    else if (this.error && !this.pending) { this.error = ""; }
    this.invalidate();
  }

  _editPat() {
    if (this.patSaved) { this.pat = ""; this.patSaved = false; this.invalidate(); }
    setTimeout(() => {
      const input = this.querySelector("#setup-pat");
      if (input) { input.focus(); }
    }, 0);
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
