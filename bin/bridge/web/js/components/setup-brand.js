import { escapeHtml } from "/assets/js/utils/escape.js";
import { t } from "/assets/js/i18n.js";

export function appNameOf(component) {
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

// The eyebrow version is the brand's own (a white-label pins its own 0.1.x),
// which says nothing about the code underneath. A screenshot has to identify
// the build, so the core version and commit ride alongside it.
function buildSuffix(component) {
  const core = component.dataset.coreVersion || "";
  return core ? ` · core ${core}` : "";
}

function buildTitle(component) {
  const core = component.dataset.coreVersion || "";
  const sha = component.dataset.gitSha || "";
  return [core && `core ${core}`, sha && `build ${sha}`].filter(Boolean).join(" · ");
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
        <span class="sp-setup__version" title="${escapeHtml(buildTitle(component))}">${escapeHtml(appName)} v${escapeHtml(version)}${escapeHtml(buildSuffix(component))}</span>
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
