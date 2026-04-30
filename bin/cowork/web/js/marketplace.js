import { $ } from "./dom.js?t=__TOKEN__";
import { api } from "./api.js?t=__TOKEN__";

const KIND_LABEL = {
  plugins: "Plugin",
  skills:  "Skill",
  hooks:   "Hook",
  mcp:     "MCP server",
  agents:  "Agent",
};
const KIND_EMPTY_TITLE = {
  plugins: "No plugins yet",
  skills:  "No skills yet",
  hooks:   "No hooks yet",
  mcp:     "No MCP servers yet",
  agents:  "No agents yet",
};

const MKT_KINDS = ["plugins", "skills", "hooks", "mcp", "agents"];

let mktData = null;
let mktKind = "plugins";
let mktSelectedId = null;
let mktSearch = "";
let mktLastSyncSummary = null;
let mktInFlight = false;

function cloneTemplate(id) {
  const tpl = $(id);
  return tpl ? tpl.content.cloneNode(true) : document.createDocumentFragment();
}

function glyphFor(kind) {
  return cloneTemplate(`tpl-mkt-glyph-${kind}`);
}

function chip(text, tone, mono) {
  const el = document.createElement("span");
  el.className = "mkt-chip" + (mono ? " mkt-chip-mono" : "");
  if (tone) el.dataset.tone = tone;
  el.textContent = text;
  return el;
}

function emptyBlock(kind, hasQuery, withSync) {
  const el = document.createElement(withSync ? "li" : "div");
  el.className = withSync ? "mkt-empty-state" : "mkt-empty";

  const glyph = document.createElement("span");
  glyph.className = "mkt-empty-glyph";
  glyph.setAttribute("aria-hidden", "true");
  glyph.append(glyphFor(kind));

  const title = document.createElement("span");
  title.className = "mkt-empty-title";
  title.textContent = hasQuery ? "No matches" : KIND_EMPTY_TITLE[kind] || "Nothing here yet";

  const sub = document.createElement("span");
  sub.className = "mkt-empty-sub";
  if (hasQuery) {
    sub.textContent = "Try a different term, or clear the search.";
  } else if (withSync) {
    sub.textContent = "Run a sync to populate the marketplace.";
  } else {
    sub.append(cloneTemplate("tpl-mkt-empty-sub-detail"));
  }

  el.append(glyph, title, sub);

  if (withSync && !hasQuery) {
    const actions = document.createElement("span");
    actions.className = "mkt-empty-actions";
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "primary";
    btn.textContent = "Sync now";
    btn.addEventListener("click", () => $("btn-sync")?.click());
    actions.append(btn);
    el.append(actions);
  }
  return el;
}

export function maybeRefreshMarketplace(snap) {
  if (!snap.signed_in) return;
  if (snap.last_sync_summary === mktLastSyncSummary && mktData) return;
  mktLastSyncSummary = snap.last_sync_summary;
  fetchMarketplace();
}

async function fetchMarketplace() {
  if (mktInFlight) return;
  mktInFlight = true;
  try {
    const resp = await api("/api/marketplace");
    if (resp.ok) {
      mktData = await resp.json();
      render();
    }
  } catch (e) {
    console.error("marketplace fetch failed", e);
  } finally {
    mktInFlight = false;
  }
}

function filterItems() {
  const items = mktData[mktKind] || [];
  if (!mktSearch) return items;
  const q = mktSearch.toLowerCase();
  return items.filter((it) =>
    (it.name || "").toLowerCase().includes(q) ||
    (it.id   || "").toLowerCase().includes(q) ||
    (it.summary || "").toLowerCase().includes(q));
}

function buildItem(it, i) {
  const li = document.createElement("li");
  li.className = "mkt-item";
  li.dataset.id = it.id;
  li.style.setProperty("--sp-mkt-item-i", String(Math.min(i, 8)));
  li.setAttribute("aria-selected", String(it.id === mktSelectedId));

  const row = document.createElement("div");
  row.className = "mkt-item-row";
  const name = document.createElement("span");
  name.className = "mkt-item-name";
  name.textContent = it.name || it.id;
  row.append(name);
  if (it.source) row.append(chip(it.source, it.source === "local" ? null : "accent"));
  li.append(row);

  if (it.summary) {
    const meta = document.createElement("div");
    meta.className = "mkt-item-meta";
    meta.textContent = it.summary;
    li.append(meta);
  }
  li.addEventListener("click", () => {
    mktSelectedId = it.id;
    render();
  });
  return li;
}

function updateCounts() {
  let total = 0;
  for (const kind of MKT_KINDS) {
    const n = (mktData[kind] || []).length;
    total += n;
    const el = document.querySelector(`.mkt-cat[data-kind="${kind}"] .mkt-cat-count`);
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
      setTimeout(() => { railTotal.dataset.bump = "false"; }, 420);
    }
  }
}

function render() {
  if (!mktData) return;
  updateCounts();
  const list = $("mkt-items");
  if (!list) return;
  const items = filterItems();
  list.replaceChildren();
  if (items.length === 0) {
    list.append(emptyBlock(mktKind, !!mktSearch, true));
  } else {
    items.forEach((it, i) => list.append(buildItem(it, i)));
  }
  renderDetail();
}

function buildDetailHead(selected) {
  const head = document.createElement("div");
  head.className = "mkt-detail-head";

  const icon = document.createElement("span");
  icon.className = "mkt-detail-icon";
  icon.setAttribute("aria-hidden", "true");
  icon.append(glyphFor(mktKind));

  const title = document.createElement("div");
  title.className = "mkt-detail-title";
  const h = document.createElement("h2");
  h.textContent = selected.name || selected.id;
  title.append(h);

  head.append(icon, title);
  return head;
}

function buildMetaRow(selected) {
  const row = document.createElement("div");
  row.className = "mkt-detail-meta";
  row.append(chip(KIND_LABEL[mktKind] || mktKind, "accent"));
  if (selected.source)  row.append(chip(selected.source));
  if (selected.version) row.append(chip("v" + selected.version, null, true));
  if (selected.license) row.append(chip(selected.license));
  return row;
}

function buildReadme(text) {
  const sec = document.createElement("section");
  sec.className = "mkt-detail-section";
  const h3 = document.createElement("h3");
  h3.textContent = "README";
  const pre = document.createElement("div");
  pre.className = "mkt-detail-readme";
  pre.textContent = text;
  sec.append(h3, pre);
  return sec;
}

function buildPathRow(pathStr) {
  const sec = document.createElement("section");
  sec.className = "mkt-detail-section";
  const h3 = document.createElement("h3");
  h3.textContent = "Path";

  const row = document.createElement("div");
  row.className = "mkt-detail-path-row";

  const folder = document.createElement("span");
  folder.setAttribute("aria-hidden", "true");
  folder.append(cloneTemplate("tpl-mkt-glyph-folder"));

  const path = document.createElement("span");
  path.className = "mkt-detail-path";
  path.textContent = pathStr;

  const copyBtn = document.createElement("button");
  copyBtn.type = "button";
  copyBtn.className = "mkt-detail-copy";
  copyBtn.textContent = "Copy";
  copyBtn.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(pathStr);
      copyBtn.dataset.copied = "true";
      copyBtn.textContent = "Copied ✓";
      setTimeout(() => {
        copyBtn.removeAttribute("data-copied");
        copyBtn.textContent = "Copy";
      }, 1200);
    } catch (e) {
      console.error("clipboard write failed", e);
    }
  });

  row.append(folder, path, copyBtn);
  sec.append(h3, row);
  return sec;
}

function renderDetail() {
  const detail = $("mkt-detail");
  if (!detail || !mktData) return;
  const items = mktData[mktKind] || [];
  const selected = items.find((it) => it.id === mktSelectedId) || null;
  detail.replaceChildren();
  if (!selected) {
    detail.append(emptyBlock(mktKind, false, false));
    return;
  }

  detail.style.animation = "none";
  void detail.offsetWidth;
  detail.style.animation = "";

  detail.append(buildDetailHead(selected));
  detail.append(buildMetaRow(selected));

  if (selected.summary) {
    const p = document.createElement("p");
    p.className = "mkt-detail-summary";
    p.textContent = selected.summary;
    detail.append(p);
  }
  if (selected.readme) detail.append(buildReadme(selected.readme));
  if (selected.path)   detail.append(buildPathRow(selected.path));
}

export function initMarketplace() {
  for (const cat of document.querySelectorAll(".mkt-cat")) {
    cat.addEventListener("click", () => {
      mktKind = cat.dataset.kind;
      mktSelectedId = null;
      for (const c of document.querySelectorAll(".mkt-cat")) {
        c.setAttribute("aria-selected", c === cat ? "true" : "false");
      }
      render();
    });
  }
  $("mkt-search")?.addEventListener("input", (e) => {
    mktSearch = e.target.value || "";
    render();
  });
  document.addEventListener("keydown", (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (!mod || e.key !== "f") return;
    const search = $("mkt-search");
    if (!search) return;
    e.preventDefault();
    search.focus();
    search.select();
  });
}
