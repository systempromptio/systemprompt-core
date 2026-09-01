import { SpElement } from "/assets/js/components/sp-element.js";
import { bridge } from "/assets/js/bridge.js";
import { probeErrorMessage, isPendingResolved, renderGatewayForm } from "/assets/js/utils/gateway.js";
import { notifyErr } from "/assets/js/utils/notify.js";
import { t } from "/assets/js/i18n.js";

const PENDING_TIMEOUT_MS = 15000;

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
    this.registerAction("sign-in", () => this._signIn());
    this.registerAction("cancel-sign-in", () => this._cancelSignIn());
    this.registerAction("connect", () => this._connect());
    this.registerAction("edit-pat", () => this._editPat());
    this.registerAction("input:gateway", (input) => this._onGatewayInput(input));
    this.registerAction("input:pat", (input) => { this.pat = input.value; });
    this.registerAction("input:keep", (input) => { this.keepSignedIn = !!input.checked; });
    this.addEventListener("focusin", (e) => {
      if (e.target.id === "setup-pat" && this.patSaved) {
        this.pat = ""; this.patSaved = false; this._syncInputs();
      }
    });
    // The URL is committed when the field is left or an action is pressed, not
    // on every keystroke: a 600ms debounce was writing half-typed URLs to disk.
    this.addEventListener("blur", (e) => {
      if (e.target && e.target.id === "setup-gateway") { this._persistGateway(); }
    }, true);
  }

  onConnect() {
    this.useSnapshot((s) => this._applySnapshot(s));
    // Sign-in is gated on a reachable gateway, so the probe has to run before
    // first paint — otherwise the button sits disabled on a stale "unknown".
    bridge.gatewayProbe().catch((e) => console.warn("initial probe", e));
  }

  onDisconnect() {
    this._clearPendingTimer();
  }

  _clearPendingTimer() {
    if (this._pendingTimer) { clearTimeout(this._pendingTimer); this._pendingTimer = null; }
  }

  _resolvePending() {
    this.pending = false; this._pendingSince = 0; this._clearPendingTimer();
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
      this._resolvePending();
    }
    const newError = probeErrorMessage(snap);
    if (newError) { this.error = newError; }
    else if (this.error && !this.pending) { this.error = ""; }
    this.invalidate();
  }

  _onGatewayInput(input) {
    this.gateway = input.value;
  }

  async _persistGateway() {
    const url = (this.gateway || "").trim();
    if (!url || url === this._lastSavedGateway) { return; }
    if (!/^https?:\/\//i.test(url)) {
      this.error = t("setup-gateway-scheme") || "Gateway URL must start with http:// or https://";
      this.invalidate();
      return;
    }
    this._lastSavedGateway = url;
    try {
      await bridge.gatewaySet(url);
    } catch (e) {
      this._lastSavedGateway = "";
      this.error = `${t("setup-gateway-save-failed") || "Could not save the gateway URL"}: ${(e && e.message) || e}`;
      this.invalidate();
      notifyErr(e, t("setup-gateway-save-failed") || "Could not save the gateway URL");
    }
  }

  _editPat() {
    if (this.patSaved) { this.pat = ""; this.patSaved = false; this.invalidate(); }
    setTimeout(() => {
      const input = this.querySelector("#setup-pat");
      if (input) { input.focus(); }
    }, 0);
  }

  _validGateway() {
    const gw = (this.gateway || "").trim();
    if (!gw) { this.error = t("setup-gateway-required-url") || "Enter the gateway URL."; this.invalidate(); return null; }
    if (!/^https?:\/\//i.test(gw)) {
      this.error = t("setup-gateway-scheme") || "Gateway URL must start with http:// or https://"; this.invalidate(); return null;
    }
    // A local gateway is plain HTTP; an https:// typo here is saved verbatim
    // and then opened in the browser, which fails with a TLS protocol error.
    if (/^https:\/\/(localhost|127\.0\.0\.1|\[::1\])(:|\/|$)/i.test(gw)) {
      this.error = t("setup-gateway-loopback-https")
        || 'A gateway on this machine is served over http://, not https:// — drop the "s".';
      this.invalidate(); return null;
    }
    return gw;
  }

  async _signIn() {
    const gw = this._validGateway();
    if (!gw) { return; }
    // Read the checkbox live so the choice is honored even if no change event fired.
    const keepEl = this.querySelector("#setup-keep");
    this.keepSignedIn = keepEl ? keepEl.checked : this.keepSignedIn;
    this._lastSavedGateway = gw;
    this.signingIn = true; this.error = ""; this.invalidate();
    try {
      await bridge.signIn(gw, this.keepSignedIn);
    } catch (err) {
      this.error = `Sign-in failed: ${(err && err.message) || err}`;
    } finally {
      this.signingIn = false; this.invalidate();
    }
  }

  async _cancelSignIn() {
    try {
      await bridge.cancel("login");
    } catch (err) {
      console.warn("cancel sign-in", err);
    }
    this.signingIn = false; this.error = ""; this.invalidate();
  }

  async _connect() {
    const gw = this._validGateway();
    if (!gw) { return; }
    this._lastSavedGateway = gw;
    this.pending = true; this._pendingSince = Date.now(); this.error = ""; this.invalidate();
    this._clearPendingTimer();
    this._pendingTimer = setTimeout(() => {
      if (!this.pending) { return; }
      this._resolvePending();
      if (!this.error) { this.error = "Connection attempt timed out."; }
      this.invalidate();
    }, PENDING_TIMEOUT_MS);
    try {
      if (this.patSaved) { await bridge.gatewayProbe(); }
      else {
        const token = (this.pat || "").trim();
        if (!token) { this.error = "Paste your personal access token."; this._resolvePending(); this.invalidate(); return; }
        await bridge.login(token, gw);
      }
    } catch (err) {
      this.error = `${this.patSaved ? "Probe" : "Login"} failed: ${(err && err.message) || err}`;
      this._resolvePending(); this.invalidate();
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
