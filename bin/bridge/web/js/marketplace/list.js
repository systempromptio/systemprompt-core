import { $ } from "../dom.js?t=__TOKEN__";
import { mktState, MKT_KINDS, KIND_EMPTY_TITLE, filterItems } from "./state.js?t=__TOKEN__";
import { chip, emptyBlock } from "./glyph.js?t=__TOKEN__";

function buildItem(it, i) {
  const li = document.createElement("li");
  li.className = "sp-mkt-item";
  li.dataset.id = it.id;
  li.dataset.action = "mkt-item";
  li.style.setProperty("--sp-mkt-item-i", String(Math.min(i, 8)));
  li.setAttribute("aria-selected", String(it.id === mktState.selectedId));

  const row = document.createElement("div");
  row.className = "sp-mkt-item__row";
  const name = document.createElement("span");
  name.className = "sp-mkt-item__name";
  name.textContent = it.name || it.id;
  row.append(name);
  if (it.source) {
    row.append(chip(it.source, it.source === "local" ? null : "accent"));
  }
  li.append(row);

  if (it.summary) {
    const meta = document.createElement("div");
    meta.className = "sp-mkt-item__meta";
    meta.textContent = it.summary;
    li.append(meta);
  }
  return li;
}

export function updateCounts() {
  if (!mktState.data) {
    return;
  }
  let total = 0;
  for (const kind of MKT_KINDS) {
    const n = (mktState.data[kind] || []).length;
    total += n;
    const el = document.querySelector(`.sp-mkt-cat[data-kind="${kind}"] .sp-mkt-cat__count`);
    if (el) {
      el.textContent = String(n);
      el.classList.toggle("is-zero", n === 0);
    }
  }
  const railTotal = $("rail-count-marketplace");
  if (railTotal) {
    const next = String(total);
    if (railTotal.textContent !== next) {
      railTotal.textContent = next;
      railTotal.dataset.bump = "true";
      railTotal.addEventListener("animationend", () => { railTotal.dataset.bump = "false"; }, { once: true });
    }
  }
}

export function renderList() {
  const list = $("mkt-items");
  if (!list || !mktState.data) {
    return;
  }
  const items = filterItems();
  list.replaceChildren();
  if (items.length === 0) {
    list.append(emptyBlock(mktState.kind, !!mktState.search, true, KIND_EMPTY_TITLE));
  } else {
    items.forEach((it, i) => list.append(buildItem(it, i)));
  }
}

export function syncCategorySelection() {
  for (const cat of document.querySelectorAll(".sp-mkt-cat")) {
    cat.setAttribute("aria-selected", cat.dataset.kind === mktState.kind ? "true" : "false");
  }
}
