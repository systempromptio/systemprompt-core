import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";

const CHANGE_L10N = {
  installed: "marketplace-change-installed",
  updated: "marketplace-change-updated",
  removed: "marketplace-change-removed",
};

const CHANGE_LABEL = {
  installed: "New",
  updated: "Updated",
  removed: "Removed",
};

export function changeBadge(change) {
  if (!change || !CHANGE_LABEL[change]) { return ""; }
  const label = t(CHANGE_L10N[change]) || CHANGE_LABEL[change];
  return `<span class="sp-mkt-chip sp-mkt-chip--change" data-change-kind="${change}">${escapeHtml(label)}</span>`;
}

export function filterItems(items, search) {
  if (!search) { return items; }
  const q = search.toLowerCase();
  return items.filter((it) =>
    (it.name || "").toLowerCase().includes(q) ||
    (it.id || "").toLowerCase().includes(q) ||
    (it.summary || "").toLowerCase().includes(q));
}

export function groupItems(items, pluginNames) {
  const groups = new Map();
  const push = (key, item) => {
    if (!groups.has(key)) { groups.set(key, []); }
    groups.get(key).push(item);
  };
  for (const item of items) {
    const owners = item.plugins || [];
    // An item two plugins ship appears under each — that is the truth of the
    // grant, and deduping it to one header would misreport who ships it. An
    // ownerless item lands under "" so the list never silently hides it.
    if (owners.length === 0) { push("", item); continue; }
    for (const owner of owners) { push(owner, item); }
  }
  const named = [...groups.keys()].filter((k) => k !== "").sort((a, b) =>
    (pluginNames[a] || a).localeCompare(pluginNames[b] || b));
  const ordered = groups.has("") ? [...named, ""] : named;
  return ordered.map((key) => ({
    key,
    label: key ? (pluginNames[key] || key) : (t("marketplace-group-ungrouped") || "Ungrouped"),
    items: groups.get(key),
  }));
}
