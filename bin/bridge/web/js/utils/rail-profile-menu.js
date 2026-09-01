import { escapeHtml } from "/assets/js/components/sp-element.js";
import { t } from "/assets/js/i18n.js";

// Why: the rail is a scroll container (`.sp-rail { overflow-y: auto }`), so
// the absolutely-positioned menu opening upward gets clipped into the rail's
// scroll overflow on short windows — leaving "Log out" rendered but
// unreachable. Re-anchor the menu to the viewport (fixed) so no ancestor
// overflow can ever swallow it; the stylesheet's absolute placement remains
// only as a fallback for the frame this runs in. Called on resize and scroll
// too, since a fixed menu does not travel with its trigger.
export function positionRailProfileMenu(component) {
  const menu = component.querySelector(".sp-rail-profile__menu");
  const trigger = component.querySelector(".sp-rail-profile__trigger");
  if (!menu || !trigger) { return; }
  const r = trigger.getBoundingClientRect();
  menu.style.position = "fixed";
  menu.style.left = `${Math.max(4, r.left)}px`;
  menu.style.right = "auto";
  menu.style.bottom = `${Math.max(4, window.innerHeight - r.top + 4)}px`;
  menu.style.minWidth = `${Math.max(140, r.width)}px`;
}

function menuItem(action, label) {
  return `<button class="sp-rail-profile__menu-item" type="button" role="menuitem" data-action="${action}">${escapeHtml(label)}</button>`;
}

function railProfileMenuItems(update) {
  const items = [];
  if (update.can_install) {
    items.push(menuItem("update-install", `${t("rail-profile-update-to") || "Update to"} v${update.version}`));
  }
  if (update.can_restart) {
    items.push(menuItem("update-restart", t("rail-profile-restart-cta") || "Restart to finish updating"));
  }
  if (update.can_install && update.notes_url) {
    items.push(`<a class="sp-rail-profile__menu-item" role="menuitem" href="${escapeHtml(update.notes_url)}" data-href="${escapeHtml(update.notes_url)}" data-action="open-external">${escapeHtml(t("rail-profile-release-notes") || "Release notes")}</a>`);
  }
  items.push(`<button class="sp-rail-profile__menu-item" type="button" role="menuitem" data-action="logout" data-l10n-id="rail-profile-logout">${escapeHtml(t("rail-profile-logout") || "Log out")}</button>`);
  return items;
}

// Only a signed-in session has anything to offer here, so the caller passes
// `open` already gated on sign-in rather than opening an empty menu.
export function renderRailProfileMenu(update, open, logoutError) {
  if (!open) { return ""; }
  return `
    <div class="sp-rail-profile__menu" role="menu">
      ${railProfileMenuItems(update).join("")}
      ${logoutError ? `<p class="sp-rail-profile__menu-error">${escapeHtml(logoutError)}</p>` : ""}
    </div>
  `;
}

// An available or installed update turns the whole control into the call to
// action; the identity and Log out stay reachable through the menu, because
// this is the only place either is offered. The CTA is its own button rather
// than a re-labelled trigger so the menu (and Log out with it) keeps a target
// of its own.
export function renderRailProfileCta(update, signedIn) {
  if (!signedIn) { return ""; }
  let cta = null;
  if (update.can_install) {
    cta = { action: "update-install", label: t("rail-profile-update-cta") || "Click here to update" };
  } else if (update.can_restart) {
    cta = { action: "update-restart", label: t("rail-profile-restart-cta") || "Restart to finish updating" };
  }
  if (!cta) { return ""; }
  return `
    <button class="sp-rail-profile__cta" type="button" data-action="${cta.action}">
      <span class="sp-rail-profile__cta-label">${escapeHtml(cta.label)}</span>
      <span class="sp-rail-profile__cta-sub">${escapeHtml(`v${update.version}`)}</span>
    </button>
  `;
}

// Why: the visible identity lives in `.sp-rail-profile__meta`, which the
// icon-only breakpoint sets to `display: none` -- that removes it from the
// accessibility tree, not just from view, leaving the button unnamed. The
// name has to be on the button itself.
export function railProfileTriggerLabel(signedIn, idLabel) {
  const base = t("rail-profile-aria") || "Account and workspace";
  return signedIn ? `${base} — ${idLabel}` : base;
}
