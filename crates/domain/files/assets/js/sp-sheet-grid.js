import { DocumentsElement } from "./documents-element.js";
import { getProjection, opId, postEdits } from "./documents-api.js";
import { attr, errorText, escapeHtml } from "./documents-html.js";

const A1 = /([A-Z]+)(\d+)$/;

const cellRef = (node) => (node.attrs?.path ?? node.id).split("/").pop();

const columnIndex = (letters) => [...letters].reduce((n, c) => n * 26 + c.charCodeAt(0) - 64, 0);

const columnName = (index) => {
  let name = "";
  for (let n = index; n > 0; n = Math.floor((n - 1) / 26)) { name = String.fromCharCode(65 + ((n - 1) % 26)) + name; }
  return name;
};

const parseValue = (raw) => {
  const text = raw.trim();
  if (text === "") { return null; }
  if (text === "true" || text === "false") { return text === "true"; }
  const number = Number(text);
  return Number.isFinite(number) ? number : raw;
};

const gridOf = (sheet) => {
  const cells = new Map();
  let cols = 0;
  let rows = 0;
  for (const cell of (sheet.children ?? []).flatMap((row) => row.children ?? [])) {
    const match = A1.exec(cellRef(cell));
    if (!match) { continue; }
    const col = columnIndex(match[1]);
    const row = Number(match[2]);
    cols = Math.max(cols, col);
    rows = Math.max(rows, row);
    cells.set(`${columnName(col)}${row}`, cell);
  }
  return { cells, cols, rows };
};

export class SpSheetGrid extends DocumentsElement {
  onConnect() {
    this.sheetIndex = 0;
    this.on("action:sheet", (el) => { this.sheetIndex = Number(el.dataset.index); this.invalidate(); });
    this.on("change:cell", (el) => this.save(el));
    this.onDocumentEvent((name) => { if (name === "document.revision.created") { this.load(); } });
    this.load();
  }

  async load() {
    try {
      const { revision_id: revision, projection } = await getProjection(this.documentId, { view: "outline" });
      this.revision = revision;
      this.sheets = projection.root.filter((node) => node.kind?.type === "sheet");
      this.error = null;
    } catch (err) {
      this.error = err;
    }
    this.invalidate();
  }

  async save(input) {
    const sheet = this.sheets[this.sheetIndex];
    const { ref } = input.dataset;
    const op = { op: "set_cells", sheet: sheet.text, range: ref, values: [[parseValue(input.value)]] };
    if (input.dataset.hash) { op.expected_hash = input.dataset.hash; }
    try {
      await postEdits(this.documentId, { baseRevision: this.revision, ops: [op], message: `set ${ref}`, opId: opId() });
      this.error = null;
    } catch (err) {
      this.error = err;
      this.invalidate();
    }
  }

  renderTabs() {
    return (this.sheets ?? []).map((sheet, index) => `<button type="button" role="tab" class="sp-sheet-grid__tab${index === this.sheetIndex ? " is-active" : ""}" data-action="sheet"${attr("data-index", index)}${attr("data-key", sheet.id)}${attr("aria-selected", String(index === this.sheetIndex))}>${escapeHtml(sheet.text)}</button>`).join("");
  }

  renderRow(row, grid) {
    const cells = [];
    for (let col = 1; col <= grid.cols + 1; col += 1) {
      const ref = `${columnName(col)}${row}`;
      const cell = grid.cells.get(ref);
      const title = cell?.attrs?.formula ? `=${cell.attrs.formula}` : ref;
      cells.push(`<td class="sp-sheet-grid__cell"${attr("data-key", ref)}><input class="sp-sheet-grid__input" data-change="cell"${attr("data-ref", ref)}${attr("data-hash", cell?.hash8)}${attr("value", cell?.text ?? "")}${attr("aria-label", ref)}${attr("title", title)}></td>`);
    }
    return `<tr${attr("data-key", `r${row}`)}><th scope="row" class="sp-sheet-grid__head">${row}</th>${cells.join("")}</tr>`;
  }

  render() {
    const error = this.error ? `<p class="sp-sheet-grid__error" role="alert">${escapeHtml(errorText(this.error))}</p>` : "";
    const sheet = this.sheets?.[this.sheetIndex];
    if (!sheet) { return `<section class="sp-sheet-grid">${error}</section>`; }
    const grid = gridOf(sheet);
    const heads = Array.from({ length: grid.cols + 1 }, (_, i) => `<th scope="col" class="sp-sheet-grid__head">${columnName(i + 1)}</th>`).join("");
    const rows = Array.from({ length: grid.rows + 1 }, (_, i) => this.renderRow(i + 1, grid)).join("");
    const truncated = sheet.attrs?.truncated ? `<p class="sp-sheet-grid__note">Large sheet: only the first cells are shown.</p>` : "";
    return `<section class="sp-sheet-grid">
      <div class="sp-sheet-grid__tabs" role="tablist">${this.renderTabs()}</div>${error}${truncated}
      <div class="sp-sheet-grid__scroll"><table class="sp-sheet-grid__table"><thead><tr><th class="sp-sheet-grid__head"></th>${heads}</tr></thead><tbody>${rows}</tbody></table></div>
    </section>`;
  }
}

customElements.define("sp-sheet-grid", SpSheetGrid);
