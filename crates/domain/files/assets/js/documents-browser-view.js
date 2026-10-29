import { attr, escapeHtml, formatBytes, formatTime } from "./documents-html.js";

const SCOPES = [["mine", "Mine"], ["shared", "Shared with me"], ["recent", "Recent"]];

function extensionsOf(extensions, kind, item) {
  let attrs = "";
  let html = "";
  for (const extend of extensions) {
    const out = extend({ kind, item }) ?? {};
    for (const [name, value] of Object.entries(out.attrs ?? {})) {
      if (name.startsWith("data-")) { attrs += attr(name, value); }
    }
    html += out.html ?? "";
  }
  return { attrs, slot: html ? `<span class="sp-doc-browser__ext">${html}</span>` : "" };
}

export function renderScopes(current) {
  return SCOPES.map(([scope, label]) => `<button type="button" class="sp-doc-browser__scope${scope === current ? " is-active" : ""}" data-action="scope"${attr("data-scope", scope)}${attr("aria-pressed", String(scope === current))}>${escapeHtml(label)}</button>`).join("");
}

export function renderCrumbs(path) {
  const crumbs = [{ name: "All documents" }, ...path].map((crumb, depth) =>
    `<li class="sp-doc-browser__crumb" data-key="crumb-${depth}"><button type="button" class="sp-doc-browser__link" data-action="crumb"${attr("data-depth", depth)}>${escapeHtml(crumb.name)}</button></li>`);
  return `<ol class="sp-doc-browser__crumbs" aria-label="Folder path">${crumbs.join("")}</ol>`;
}

export function renderMenu(items, folder, isOpen) {
  if (!items.length) { return ""; }
  const toggle = `<button type="button" class="sp-doc-browser__more" data-action="folder-menu"${attr("data-id", folder.id)}${attr("aria-expanded", String(isOpen))} aria-label="More">⋯</button>`;
  if (!isOpen) { return toggle; }
  const entries = items.map((item) => `<li role="none"><button type="button" role="menuitem" class="sp-doc-browser__menu-item" data-action="folder-menu-item"${attr("data-item", item.id)}>${escapeHtml(item.label)}</button></li>`).join("");
  return `${toggle}<ul class="sp-doc-browser__menu" role="menu">${entries}</ul>`;
}

export function renderFolder(extensions, folder, menu) {
  const ext = extensionsOf(extensions, "folder", folder);
  return `<li class="sp-doc-browser__row sp-doc-browser__row--folder"${attr("data-key", `f-${folder.id}`)}${attr("data-folder-row", folder.id)}${ext.attrs}>
      <button type="button" class="sp-doc-browser__link" data-action="folder"${attr("data-id", folder.id)}${attr("data-name", folder.name)}>${escapeHtml(folder.name)}/</button>
      ${ext.slot}<span class="sp-doc-browser__meta">${escapeHtml(formatTime(folder.updated_at))}</span>${menu}</li>`;
}

export function renderDocument(extensions, item) {
  const access = item.my_access ? `<span class="sp-doc-browser__badge">${escapeHtml(item.my_access)}</span>` : "";
  const ext = extensionsOf(extensions, "document", item);
  return `<li class="sp-doc-browser__row"${attr("data-key", `d-${item.id}`)}${attr("data-document-row", item.id)}${ext.attrs}>
      <button type="button" class="sp-doc-browser__link" data-action="open"${attr("data-id", item.id)}${attr("data-format", item.format)}${attr("data-name", item.name)}>${escapeHtml(item.name)}</button>
      ${ext.slot}${access}<span class="sp-doc-browser__meta">${escapeHtml(formatBytes(item.size_bytes))}</span>
      <span class="sp-doc-browser__meta">${escapeHtml(formatTime(item.updated_at))}</span></li>`;
}
