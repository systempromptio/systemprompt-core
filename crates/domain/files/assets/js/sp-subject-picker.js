import { DocumentsElement } from "./documents-element.js";
import { searchSubjects } from "./documents-api.js";
import { attr, errorText, escapeHtml } from "./documents-html.js";

const DEBOUNCE_MS = 200;
const LIMIT = 20;

/*
 * A grant-subject typeahead inside a share form. It contributes two form
 * controls, `kind` and `value`: `value` is the subject value a grant names
 * (a user id), while the visible box shows the display name. A kind whose
 * values cannot be listed answers `searchable: false`; the typed text is then
 * the value as-is.
 */
export class SpSubjectPicker extends DocumentsElement {
  onConnect() {
    this.kind = this.kinds[0] ?? "user";
    this.text = "";
    this.value = "";
    this.matches = [];
    this.active = -1;
    this.isSearchable = true;
    this.on("change:kind", (el) => this.setKind(el.value));
    this.on("input:query", (el) => this.typed(el.value));
    this.on("action:pick", (el) => this.pick(Number(el.dataset.index)));
    const keys = (evt) => this.navigate(evt);
    this.addEventListener("keydown", keys);
    this.own(() => this.removeEventListener("keydown", keys));
    this.own(() => clearTimeout(this._timer));
  }

  get kinds() {
    return (this.getAttribute("kinds") ?? "user").split(",").filter(Boolean);
  }

  setKind(kind) {
    this.kind = kind;
    this.typed("");
  }

  typed(text) {
    this.text = text;
    this.value = text.trim();
    clearTimeout(this._timer);
    this._timer = setTimeout(() => this.search(), DEBOUNCE_MS);
    this.invalidate();
  }

  async search() {
    const asked = this.text.trim();
    try {
      const found = await searchSubjects({ kind: this.kind, q: asked, limit: LIMIT });
      if (asked !== this.text.trim()) { return; }
      this.isSearchable = found.searchable;
      this.matches = found.subjects;
      this.active = this.matches.length ? 0 : -1;
      this.error = null;
    } catch (err) {
      this.error = err;
      this.matches = [];
    }
    this.invalidate();
  }

  pick(index) {
    const match = this.matches[index];
    if (!match) { return; }
    this.value = match.value;
    this.text = match.display_name;
    this.matches = [];
    this.active = -1;
    this.invalidate();
  }

  navigate(evt) {
    if (!this.matches.length) { return; }
    const moves = { ArrowDown: 1, ArrowUp: -1 };
    if (evt.key in moves) {
      evt.preventDefault();
      const count = this.matches.length;
      this.active = (this.active + moves[evt.key] + count) % count;
      this.invalidate();
    } else if (evt.key === "Enter" && this.active >= 0) {
      evt.preventDefault();
      this.pick(this.active);
    } else if (evt.key === "Escape") {
      this.matches = [];
      this.invalidate();
    }
  }

  renderOption(match, index) {
    const id = `${this.id || "subject"}-option-${index}`;
    const detail = match.detail ? ` <span class="sp-subject-picker__detail">${escapeHtml(match.detail)}</span>` : "";
    return `<li class="sp-subject-picker__option" role="option" data-action="pick"${attr("id", id)}${attr("data-key", match.value)}${attr("data-index", index)}${attr("aria-selected", String(index === this.active))}>${escapeHtml(match.display_name)}${detail}</li>`;
  }

  render() {
    const listId = `${this.id || "subject"}-listbox`;
    const kinds = this.kinds.map((k) => `<option${attr("value", k)}${attr("selected", k === this.kind)}>${escapeHtml(k)}</option>`).join("");
    const kindField = this.kinds.length > 1
      ? `<label class="sp-share-dialog__field">Subject kind<select name="kind" data-change="kind">${kinds}</select></label>`
      : `<input type="hidden" name="kind"${attr("value", this.kind)}>`;
    const open = this.matches.length > 0;
    const activeId = open && this.active >= 0 ? `${this.id || "subject"}-option-${this.active}` : null;
    const hint = this.isSearchable ? "" : `<p class="sp-subject-picker__hint">Enter the exact ${escapeHtml(this.kind)} value.</p>`;
    const error = this.error ? `<p class="sp-share-dialog__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    return `${kindField}<label class="sp-share-dialog__field">${escapeHtml(this.getAttribute("label") ?? "Subject")}
      <input name="query" role="combobox" autocomplete="off" aria-autocomplete="list" data-input="query" required${attr("value", this.text)}${attr("aria-controls", listId)}${attr("aria-expanded", String(open))}${attr("aria-activedescendant", activeId)}></label>
      <input type="hidden" name="value"${attr("value", this.value)}>
      <ul class="sp-subject-picker__options" role="listbox"${attr("id", listId)}${attr("hidden", !open)}>${this.matches.map((m, i) => this.renderOption(m, i)).join("")}</ul>${hint}${error}`;
  }
}

customElements.define("sp-subject-picker", SpSubjectPicker);
