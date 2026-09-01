import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";

const STEP_LABEL = {
  connect: () => t("setup-step-label-connect") || "Step 1 of 2",
  agents: () => t("setup-step-label-agents") || "Step 2 of 2",
};

export function renderSetupHero(component) {
  const stepLabel = (STEP_LABEL[component.step] || (() => ""))();
  const version = component.dataset.version || "";
  // Brand name comes from the snapshot (Brand::app_name), so white-label builds
  // get their own name with no forked component.
  const appName = escapeHtml((component.snapshot && component.snapshot.app_name) || "systemprompt bridge");
  return `
    <div class="sp-setup__hero">
      <div class="sp-setup__mark" data-logo-slot data-preserve></div>
      <div class="sp-setup__eyebrow"><span data-l10n-id="setup-eyebrow-prefix">DEMO BUILD</span> · v${escapeHtml(version)} · <span>${escapeHtml(stepLabel)}</span></div>
      <h1 data-l10n-id="setup-heading">Welcome to ${appName}</h1>
      <p class="sp-setup__lede" data-l10n-id="setup-lede">${appName} routes one or more coding agents through your enterprise gateway.</p>
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
  const appName = escapeHtml((component.snapshot && component.snapshot.app_name) || "systemprompt bridge");
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

export function renderSetupFooter(component) {
  const platform = component.dataset.platform || "linux";
  const platformDisplay = component.dataset.platformDisplay || "";
  const snap = component.snapshot || {};
  // Docs base + contact come from the Brand (via the snapshot) so a white-label
  // footer links to its own docs/licensing, not systemprompt's.
  const docsBase = snap.docs_url || "https://systemprompt.io/docs/bridge";
  const docsHref = escapeHtml(`${docsBase}/${platform}`);
  const email = escapeHtml(snap.contact_email || "ed@systemprompt.io");
  const appName = (snap.app_name || "systemprompt bridge");
  const subject = escapeHtml(encodeURIComponent(`${appName} licensing`));
  return `
    <aside class="sp-setup__warning" role="note">
      <strong data-l10n-id="setup-warning-strong">Demo software.</strong>
      <span data-l10n-id="setup-warning-body">This build is provided for demonstration purposes only and is not licensed for production use.</span>
    </aside>
    <p class="sp-setup__meta">
      <a class="sp-setup__docs" href="${docsHref}" target="_blank" rel="noopener noreferrer">
        Documentation for ${escapeHtml(platformDisplay)} →
      </a>
      <span class="sp-setup__meta-sep">·</span>
      <span>Licensing — <a href="mailto:${email}?subject=${subject}">${email}</a></span>
    </p>`;
}
