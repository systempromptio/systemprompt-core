import { $ } from "../dom.js?t=__TOKEN__";
import { mktState, KIND_LABEL, KIND_EMPTY_TITLE } from "./state.js?t=__TOKEN__";
import { chip, glyphFor, cloneTemplate, emptyBlock } from "./glyph.js?t=__TOKEN__";

function buildDetailHead(selected) {
  const head = document.createElement("div");
  head.className = "sp-mkt-detail__head";

  const icon = document.createElement("span");
  icon.className = "sp-mkt-detail__icon";
  icon.setAttribute("aria-hidden", "true");
  icon.append(glyphFor(mktState.kind));

  const title = document.createElement("div");
  title.className = "sp-mkt-detail__title";
  const h = document.createElement("h2");
  h.textContent = selected.name || selected.id;
  title.append(h);

  head.append(icon, title);
  return head;
}

function buildMetaRow(selected) {
  const row = document.createElement("div");
  row.className = "sp-mkt-detail__meta";
  row.append(chip(KIND_LABEL[mktState.kind] || mktState.kind, "accent"));
  if (selected.source) {
    row.append(chip(selected.source));
  }
  if (selected.version) {
    row.append(chip("v" + selected.version, null, true));
  }
  if (selected.license) {
    row.append(chip(selected.license));
  }
  return row;
}

function buildReadme(text) {
  const sec = document.createElement("section");
  sec.className = "sp-mkt-detail__section";
  const h3 = document.createElement("h3");
  h3.textContent = "README";
  const pre = document.createElement("div");
  pre.className = "sp-mkt-detail__readme";
  pre.textContent = text;
  sec.append(h3, pre);
  return sec;
}

function buildPathRow(pathStr) {
  const sec = document.createElement("section");
  sec.className = "sp-mkt-detail__section";
  const h3 = document.createElement("h3");
  h3.textContent = "Path";

  const row = document.createElement("div");
  row.className = "sp-mkt-detail__path-row";

  const folder = document.createElement("span");
  folder.setAttribute("aria-hidden", "true");
  folder.append(cloneTemplate("tpl-mkt-glyph-folder"));

  const path = document.createElement("span");
  path.className = "sp-mkt-detail__path";
  path.textContent = pathStr;

  const copyBtn = document.createElement("button");
  copyBtn.type = "button";
  copyBtn.className = "sp-mkt-detail__copy";
  copyBtn.textContent = "Copy";
  copyBtn.dataset.action = "mkt-copy";
  copyBtn.dataset.value = pathStr;

  row.append(folder, path, copyBtn);
  sec.append(h3, row);
  return sec;
}

export async function copyToClipboard(button, value) {
  try {
    await navigator.clipboard.writeText(value);
    button.dataset.copied = "true";
    button.textContent = "Copied ✓";
    await button.animate([{}, {}], { duration: 1200 }).finished;
    button.removeAttribute("data-copied");
    button.textContent = "Copy";
  } catch (e) {
    console.error("clipboard write failed", e);
  }
}

export function renderDetail() {
  const detail = $("mkt-detail");
  if (!detail || !mktState.data) {
    return;
  }
  const items = mktState.data[mktState.kind] || [];
  const selected = items.find((it) => it.id === mktState.selectedId) || null;
  detail.replaceChildren();
  if (!selected) {
    detail.append(emptyBlock(mktState.kind, false, false, KIND_EMPTY_TITLE));
  } else {
    detail.classList.remove("is-entering");
    void detail.offsetWidth;
    detail.classList.add("is-entering");

    detail.append(buildDetailHead(selected));
    detail.append(buildMetaRow(selected));

    if (selected.summary) {
      const p = document.createElement("p");
      p.className = "sp-mkt-detail__summary";
      p.textContent = selected.summary;
      detail.append(p);
    }
    if (selected.readme) {
      detail.append(buildReadme(selected.readme));
    }
    if (selected.path) {
      detail.append(buildPathRow(selected.path));
    }
  }
}
