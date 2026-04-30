import { $ } from "./dom.js?t=__TOKEN__";

export function renderProfile(snap) {
  const id = snap.verified_identity;
  const profileSub = $("rail-profile-sub");
  if (profileSub) {
    if (!profileSub.dataset.baseVersion) {
      profileSub.dataset.baseVersion = profileSub.textContent.trim();
    }
    const baseVersion = profileSub.dataset.baseVersion;
    const tenant = id && id.tenant_id;
    profileSub.textContent = tenant ? `${tenant} · ${baseVersion}` : baseVersion;
  }
  const profileId = $("rail-profile-id");
  if (profileId) {
    profileId.textContent = (id && (id.email || id.user_id)) || "cowork workspace";
  }
  const initials = $("rail-profile-initials");
  if (initials) {
    const idSrc = (id && (id.email || id.user_id)) || "";
    const letters = idSrc.replace(/[^a-zA-Z]/g, "").slice(0, 2).toUpperCase();
    initials.textContent = letters || "SP";
  }
}
