import { $ } from "../dom.js?t=__TOKEN__";
import { t } from "../i18n.js?t=__TOKEN__";

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
  el.className = "sp-mkt-chip" + (mono ? " sp-mkt-chip--mono" : "");
  if (tone) {
    el.dataset.tone = tone;
  }
  el.textContent = text;
  return el;
}

export function emptyBlock(kind, hasQuery, withSync, emptyTitleMap) {
  const el = document.createElement(withSync ? "li" : "div");
  el.className = withSync ? "sp-mkt-empty--with-sync" : "sp-mkt-empty";

  const glyph = document.createElement("span");
  glyph.className = "sp-mkt-empty__glyph";
  glyph.setAttribute("aria-hidden", "true");
  glyph.append(glyphFor(kind));

  const title = document.createElement("span");
  title.className = "sp-mkt-empty__title";
  title.textContent = hasQuery ? "No matches" : (emptyTitleMap[kind] || "Nothing here yet");

  const sub = document.createElement("span");
  sub.className = "sp-mkt-empty__sub";
  if (hasQuery) {
    sub.textContent = t("marketplace-empty-search-sub");
  } else if (withSync) {
    sub.textContent = t("marketplace-empty-presync-sub");
  } else {
    sub.append(cloneTemplate("tpl-mkt-empty-sub-detail"));
  }

  el.append(glyph, title, sub);

  if (withSync && !hasQuery) {
    const actions = document.createElement("span");
    actions.className = "sp-mkt-empty__actions";
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "sp-btn-primary";
    btn.dataset.action = "sync";
    btn.textContent = t("marketplace-empty-sync-button");
    actions.append(btn);
    el.append(actions);
  }
  return el;
}
