import { patchHtml } from "./documents-patch.js";
import { subscribeDocument } from "./documents-events.js";

/*
 * Base class for the document components (render-to-string + keyed patch).
 * `render()` returns escaped markup; re-renders are coalesced to one per
 * microtask; `data-action` clicks (and Enter/Space on non-native triggers)
 * and `data-change` / `data-submit` events dispatch to registered handlers;
 * every subscription is released on disconnect.
 */
export class DocumentsElement extends HTMLElement {
  constructor() {
    super();
    this._unsubs = [];
    this._handlers = Object.create(null);
    this._scheduled = false;
    this._connected = false;
    this._bound = false;
  }

  get documentId() {
    return this.getAttribute("document-id");
  }

  connectedCallback() {
    this._connected = true;
    this._bind();
    this.onConnect?.();
    this.invalidate();
  }

  disconnectedCallback() {
    this._connected = false;
    for (const unsub of this._unsubs) { unsub(); }
    this._unsubs = [];
    this.onDisconnect?.();
  }

  onDocumentEvent(fn) {
    this._unsubs.push(subscribeDocument(this.documentId, fn));
  }

  own(unsub) {
    this._unsubs.push(unsub);
  }

  on(name, fn) {
    this._handlers[name] = fn;
  }

  invalidate() {
    if (this._scheduled || !this._connected) { return; }
    this._scheduled = true;
    queueMicrotask(() => {
      this._scheduled = false;
      if (this._connected) { this._renderNow(); }
    });
  }

  _renderNow() {
    patchHtml(this, this.render());
    this.afterRender?.();
  }

  _dispatch(evt, kind) {
    const trigger = evt.target.closest(`[data-${kind}]`);
    if (!trigger || !this.contains(trigger)) { return; }
    const fn = this._handlers[`${kind}:${trigger.dataset[kind]}`];
    if (!fn) { return; }
    if (kind === "submit") { evt.preventDefault(); }
    fn.call(this, trigger, evt);
  }

  _bind() {
    if (this._bound) { return; }
    this._bound = true;
    this.addEventListener("click", (evt) => this._dispatch(evt, "action"));
    this.addEventListener("keydown", (evt) => {
      if (evt.key !== "Enter" && evt.key !== " ") { return; }
      const trigger = evt.target.closest("[data-action]");
      if (!trigger || ["BUTTON", "A", "SUMMARY", "INPUT"].includes(trigger.tagName)) { return; }
      evt.preventDefault();
      this._dispatch(evt, "action");
    });
    this.addEventListener("change", (evt) => this._dispatch(evt, "change"));
    this.addEventListener("input", (evt) => this._dispatch(evt, "input"));
    this.addEventListener("submit", (evt) => this._dispatch(evt, "submit"));
  }
}
