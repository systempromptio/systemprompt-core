import { $ } from "../dom.js?t=__TOKEN__";

export function cloneTemplate(id) {
  const tpl = $(id);
  if (tpl) {
    return tpl.content.cloneNode(true);
  } else {
    return document.createDocumentFragment();
  }
}

export function glyphFor(kind) {
  return cloneTemplate(`tpl-mkt-glyph-${kind}`);
}

export function chip(text, tone, mono) {
  const el = document.createElement("span");
  el.className = "mkt-chip" + (mono ? " mkt-chip-mono" : "");
  if (tone) {
    el.dataset.tone = tone;
  }
  el.textContent = text;
  return el;
}

export function emptyBlock(kind, hasQuery, withSync, emptyTitleMap) {
  const el = document.createElement(withSync ? "li" : "div");
  el.className = withSync ? "mkt-empty-state" : "mkt-empty";

  const glyph = document.createElement("span");
  glyph.className = "mkt-empty-glyph";
  glyph.setAttribute("aria-hidden", "true");
  glyph.append(glyphFor(kind));

  const title = document.createElement("span");
  title.className = "mkt-empty-title";
  title.textContent = hasQuery ? "No matches" : (emptyTitleMap[kind] || "Nothing here yet");

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
    btn.dataset.action = "sync";
    btn.textContent = "Sync now";
    actions.append(btn);
    el.append(actions);
  }
  return el;
}
