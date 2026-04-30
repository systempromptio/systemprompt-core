import { LitElement } from "/assets/js/vendor/lit-all.js";
import { subscribe } from "/assets/js/bridge.js";

export class BridgeElement extends LitElement {
  constructor() {
    super();
    this.__bridgeUnsubs = [];
  }

  bridgeSubscribe(channel, cb) {
    const unsub = subscribe(channel, cb);
    this.__bridgeUnsubs.push(unsub);
    return unsub;
  }

  disconnectedCallback() {
    for (const u of this.__bridgeUnsubs) {
      try { u(); } catch (e) { console.error("BridgeElement teardown", e); }
    }
    this.__bridgeUnsubs = [];
    super.disconnectedCallback();
  }
}
