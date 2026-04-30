import { html, nothing } from "/assets/js/vendor/lit-all.js";
import { BridgeElement } from "/assets/js/components/base.js";
import { bridge } from "/assets/js/bridge.js";
import { t } from "/assets/js/i18n.js";

const PERSIST_DEBOUNCE_MS = 600;

export class SpSetupGateway extends BridgeElement {
  static properties = {
    snapshot: { state: true },
    gateway: { state: true },
    pat: { state: true },
    patSaved: { state: true },
    error: { state: true },
    pending: { state: true },
  };

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
  }

  createRenderRoot() { return this; }

  connectedCallback() {
    super.connectedCallback();
    bridge.stateSnapshot().then((s) => this._applySnapshot(s)).catch(() => {});
    this.bridgeSubscribe("state.changed", (s) => this._applySnapshot(s));
  }

  disconnectedCallback() {
    if (this._debounce) { clearTimeout(this._debounce); }
    super.disconnectedCallback();
  }

  _applySnapshot(snap) {
    this.snapshot = snap;
    if (snap && document.activeElement && document.activeElement.id === "setup-gateway") {
      // user is editing — don't overwrite
    } else if (snap && snap.gateway_url && this.gateway !== snap.gateway_url) {
      this.gateway = snap.gateway_url;
      this._lastSavedGateway = snap.gateway_url;
    }
    if (snap) {
      if (snap.pat_present && !this.patSaved && !this.pat) {
        this.pat = "•".repeat(24);
        this.patSaved = true;
      } else if (!snap.pat_present && this.patSaved) {
        this.pat = "";
        this.patSaved = false;
      }
    }
    this._maybeClearPending(snap);
    this._syncProbeError(snap);
  }

  _maybeClearPending(snap) {
    if (!this.pending) { return; }
    if (!snap) { return; }
    const probeState = (snap.gateway_status && snap.gateway_status.state) || "unknown";
    const configured = probeState === "reachable" && snap.verified_identity && snap.verified_identity.user_id;
    const elapsed = this._pendingSince > 0 ? (Date.now() - this._pendingSince) : 0;
    if (configured || probeState === "unreachable" || elapsed > 15000) {
      this.pending = false;
      this._pendingSince = 0;
    }
  }

  _syncProbeError(snap) {
    if (!snap) { return; }
    const status = snap.gateway_status || { state: "unknown" };
    const verified = snap.verified_identity && snap.verified_identity.user_id;
    if (status.state === "reachable" && snap.pat_present && !verified) {
      this.error = "Token rejected by gateway. Issue a fresh PAT and try again.";
    } else if (status.state === "unreachable" && snap.pat_present) {
      this.error = `Gateway unreachable: ${status.reason || "unknown error"}`;
    } else if (this.error && !this.pending) {
      this.error = "";
    }
  }

  _onGatewayInput(e) {
    this.gateway = e.target.value;
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

  _onPatFocus() {
    if (this.patSaved) { this.pat = ""; this.patSaved = false; }
  }
  _onPatInput(e) {
    this.pat = e.target.value;
  }

  _editPat(e) {
    e.stopPropagation();
    if (this.patSaved) { this.pat = ""; this.patSaved = false; this.requestUpdate(); }
    setTimeout(() => {
      const input = this.querySelector("#setup-pat");
      if (input) { input.focus(); }
    }, 0);
  }

  async _connect(e) {
    e.stopPropagation();
    const gw = (this.gateway || "").trim();
    if (!gw) { this.error = "Enter the gateway URL."; return; }
    if (!/^https?:\/\//i.test(gw)) { this.error = "Gateway URL must start with http:// or https://"; return; }
    if (this.patSaved) {
      this.error = "";
      this._lastSavedGateway = gw;
      this.pending = true;
      this._pendingSince = Date.now();
      try { await bridge.gatewayProbe(); }
      catch (err) {
        this.error = `Probe failed: ${(err && err.message) || err}`;
        this.pending = false;
        this._pendingSince = 0;
      }
      return;
    }
    const token = (this.pat || "").trim();
    if (!token) { this.error = "Paste your personal access token."; return; }
    this.error = "";
    this._lastSavedGateway = gw;
    this.pending = true;
    this._pendingSince = Date.now();
    try { await bridge.login(token, gw); }
    catch (err) {
      this.error = `Login failed: ${(err && err.message) || err}`;
      this.pending = false;
      this._pendingSince = 0;
    }
  }

  _patLink() {
    const gw = (this.gateway || "").trim().replace(/\/+$/, "");
    if (gw) { return `${gw}/admin/login`; }
    return "#";
  }

  _probeView() {
    const snap = this.snapshot || {};
    const status = snap.gateway_status || { state: "unknown" };
    if (status.state === "reachable") {
      return { dot: "sp-dot--ok", muted: false, text: t("setup-gateway-reachable", { latency: status.latency_ms }) || `reachable · ${status.latency_ms}ms` };
    }
    if (status.state === "probing") {
      return { dot: "sp-dot--probing", muted: true, text: t("setup-gateway-probing") || "probing…" };
    }
    if (status.state === "unreachable") {
      return { dot: "sp-dot--err", muted: false, text: t("setup-gateway-unreachable", { reason: status.reason || "unknown" }) || `unreachable · ${status.reason || "unknown"}` };
    }
    const empty = !snap.gateway_url;
    return { dot: "sp-dot--unknown", muted: true, text: empty ? (t("setup-gateway-empty") || "enter a URL to probe…") : (t("setup-gateway-not-probed") || "not probed yet") };
  }

  render() {
    const probe = this._probeView();
    const patLink = this._patLink();
    const linkDisabled = patLink === "#";
    return html`
      <div class="sp-setup__field">
        <label for="setup-gateway" data-l10n-id="setup-gateway-label">Gateway URL</label>
        <input id="setup-gateway" type="url" placeholder="http://127.0.0.1:8080" autocomplete="off" spellcheck="false" .value=${this.gateway} @input=${(e) => this._onGatewayInput(e)} @blur=${() => this._onGatewayBlur()} />
        <div class="sp-setup__status">
          <span class="sp-dot ${probe.dot}" aria-hidden="true"></span>
          <span class=${probe.muted ? "sp-u-muted" : ""}>${probe.text}</span>
        </div>
      </div>
      <div class="sp-setup__field">
        <label for="setup-pat" data-l10n-id="setup-pat-label">Personal access token</label>
        <input id="setup-pat" type="password" placeholder="sp-live-…" autocomplete="off" spellcheck="false" .value=${this.pat} @focus=${() => this._onPatFocus()} @input=${(e) => this._onPatInput(e)} />
        <p class="sp-setup__hint">
          <span data-l10n-id="setup-pat-hint">Don't have one yet?</span>
          <a class="sp-setup__pat-link ${linkDisabled ? "is-disabled" : ""}" href=${patLink} target="_blank" rel="noopener noreferrer" aria-disabled=${linkDisabled ? "true" : "false"} @click=${(e) => { if (linkDisabled) { e.preventDefault(); } }}>Open the gateway admin login →</a>
          ${this.patSaved ? html`<button class="sp-btn-ghost" type="button" @click=${(e) => this._editPat(e)}>Edit</button>` : nothing}
        </p>
      </div>
      <div class="sp-setup__actions">
        <button class="sp-btn-primary" type="button" ?disabled=${this.pending} @click=${(e) => this._connect(e)}>
          <span class="sp-btn__label">${this.pending ? (t("setup-connecting") || "Connecting…") : "Connect"}</span>
        </button>
        ${this.error ? html`<span class="sp-setup__error">${this.error}</span>` : nothing}
      </div>
    `;
  }
}

customElements.define("sp-setup-gateway", SpSetupGateway);
