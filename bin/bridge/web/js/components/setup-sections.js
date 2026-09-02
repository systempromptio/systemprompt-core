import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";

const STEP_LABEL = {
  connect: () => t("setup-step-label-connect") || "Step 1 of 2",
  agents: () => t("setup-step-label-agents") || "Step 2 of 2",
};

function appNameOf(component) {
  // Brand name comes from the snapshot (Brand::app_name), so white-label builds
  // get their own name with no forked component.
  return (component.snapshot && component.snapshot.app_name) || "systemprompt bridge";
}

// Left column of the setup split: the brand mark, its pitch, and the
// docs/licensing footer. The pitch comes from the snapshot
// (Brand::pitch_head/body) so a white-label build states its own value
// proposition with no forked component.
export function renderSetupBrand(component) {
  const snap = component.snapshot || {};
  const pitchHead = escapeHtml(snap.pitch_head || "Govern every coding agent.");
  const pitchBody = escapeHtml(
    snap.pitch_body || "One gateway. Every agent. Every tool call audited.",
  );
  return `
    <aside class="sp-setup__brand">
      <div class="sp-setup__mark" data-logo-slot data-preserve></div>
      <div class="sp-setup__pitch">
        <p class="sp-setup__pitch-head">${pitchHead}</p>
        <p class="sp-setup__pitch-body">${pitchBody}</p>
      </div>
      ${renderSetupBrandFoot(component)}
    </aside>`;
}

function renderSetupBrandFoot(component) {
  const version = component.dataset.version || "";
  const platform = component.dataset.platform || "linux";
  const platformDisplay = component.dataset.platformDisplay || "";
  const snap = component.snapshot || {};
  const appName = appNameOf(component);
  // Docs base + contact come from the Brand (via the snapshot) so a white-label
  // footer links to its own docs/licensing, not systemprompt's.
  const docsBase = snap.docs_url || "https://systemprompt.io/docs/bridge";
  const docsHref = escapeHtml(`${docsBase}/${platform}`);
  const email = escapeHtml(snap.contact_email || "ed@systemprompt.io");
  const subject = escapeHtml(encodeURIComponent(`${appName} licensing`));
  return `
    <footer class="sp-setup__brand-foot">
      <p class="sp-setup__demo">
        <strong data-l10n-id="setup-warning-strong">Demo software.</strong>
        <span data-l10n-id="setup-warning-body">This build is provided for demonstration purposes only and is not licensed for production use.</span>
      </p>
      <p class="sp-setup__meta">
        <span class="sp-setup__version">${escapeHtml(appName)} v${escapeHtml(version)}</span>
        <span class="sp-setup__meta-sep">·</span>
        <a class="sp-setup__docs" href="${docsHref}" target="_blank" rel="noopener noreferrer">
          Documentation for ${escapeHtml(platformDisplay)} →
        </a>
        <span class="sp-setup__meta-sep">·</span>
        <span>Licensing — <a href="mailto:${email}?subject=${subject}">${email}</a></span>
      </p>
    </footer>`;
}

// Right column heading: the eyebrow (build tag + step) and the welcome line
// that sits above whichever step is active.
export function renderSetupHeading(component) {
  const stepLabel = (STEP_LABEL[component.step] || (() => ""))();
  const version = component.dataset.version || "";
  const appName = escapeHtml(appNameOf(component));
  return `
    <div class="sp-setup__heading">
      <div class="sp-setup__eyebrow"><span data-l10n-id="setup-eyebrow-prefix">DEMO BUILD</span> · v${escapeHtml(version)} · <span>${escapeHtml(stepLabel)}</span></div>
      <h1 data-l10n-id="setup-heading">Welcome to ${appName}</h1>
      <p class="sp-setup__lede" data-l10n-id="setup-lede">${appName} routes one or more coding agents through your enterprise gateway.</p>
    </div>`;
}

// Shown after sign-in succeeds while the bridge is still probing hosts, syncing
// and writing policy — the window that previously flashed the sign-in form or a
// full agent picker and read as broken.
export function renderSetupFinalizing(component) {
  const appName = escapeHtml(appNameOf(component));
  return `
    <div class="sp-setup__finalizing" role="status" aria-live="polite">
      <span class="sp-btn__spinner sp-setup__finalizing-spinner" aria-hidden="true"></span>
      <div>
        <p class="sp-setup__finalizing-head">${escapeHtml(t("setup-finalizing-head") || "Finishing setup…")}</p>
        <p class="sp-setup__finalizing-body">${escapeHtml(t("setup-finalizing-body", { app: component.snapshot ? appNameOf(component) : "the bridge" }) || "Signing you in and preparing the bridge. This only takes a moment.")}</p>
      </div>
    </div>`;
}

export function renderSetupAgentsStep(component) {
  // Finish is enabled except while first-use provisioning is running. Host
  // install-state is probe-driven and can lag or misreport (e.g. the card
  // shows "Installed ✓" while `anyInstalled` is still false), which trapped
  // the user on this step with no way forward — so it is never gated on that.
  // An in-flight run is different: it is a bounded operation that reports its
  // own completion, and leaving mid-run is what produced a broken app.
  const finishDisabled = component.firstRunActive ? "disabled" : "";
  const confirm = component.confirmEmptyFinish;
  const finishButton = confirm
    ? `<button class="sp-btn-primary" type="button" data-action="finish-anyway">${escapeHtml(t("setup-finish-anyway") || "Finish anyway")}</button>`
    : `<button class="sp-btn-primary" type="button" data-l10n-id="setup-finish" data-action="finish" ${finishDisabled}>Finish</button>`;
  const appName = escapeHtml(appNameOf(component));
  return `
    <div class="sp-setup__step" data-step="agents" ${component.step !== "agents" ? "hidden" : ""}>
      <p class="sp-setup__lede" data-l10n-id="setup-agents-lede">Pick the coding agents you want ${appName} to govern.</p>
      <sp-setup-agents></sp-setup-agents>
      ${confirm
        ? `<p class="sp-setup__note" role="alert">${escapeHtml(t("setup-finish-empty-warning") || `You have not added an agent yet, so nothing will be routed through ${appName}.`)}</p>`
        : ""}
      <div class="sp-setup__actions">${finishButton}</div>
    </div>`;
}

export function renderSetupSettleNotice(component) {
  if (!component.settleTimedOut) { return ""; }
  const snap = component.snapshot || {};
  const unreachable = snap.gateway_status && snap.gateway_status.tone === "err";
  const line = unreachable
    ? t("setup-settle-unreachable", { gateway: snap.gateway_url || "" })
    : t("setup-settle-slow") || "Still checking this computer. Some agents have not reported yet.";
  return `
    <div class="sp-setup__note sp-setup__note--warn" role="alert">
      <p>${escapeHtml(line)}</p>
      <div class="sp-setup__actions">
        <button class="sp-btn-ghost" type="button" data-action="retry-settle">${escapeHtml(t("setup-retry") || "Check again")}</button>
        <button class="sp-btn-ghost" type="button" data-action="continue-anyway">${escapeHtml(t("setup-continue-anyway") || "Continue anyway")}</button>
      </div>
    </div>`;
}
