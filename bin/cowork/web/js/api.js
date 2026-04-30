export const API_TOKEN = window.__SP_TOKEN__ || "";

function withToken(path) {
  const sep = path.includes("?") ? "&" : "?";
  return `${path}${sep}t=${encodeURIComponent(API_TOKEN)}`;
}

export async function apiGet(path) {
  const resp = await fetch(withToken(path));
  if (resp.ok) {
    return resp.json();
  } else {
    throw new Error(`GET ${path} failed: ${resp.status}`);
  }
}

export async function apiPost(path, body) {
  const init = {
    method: "POST",
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  };
  const resp = await fetch(withToken(path), init);
  if (!resp.ok && resp.status !== 204) {
    const text = await resp.text();
    throw new Error(`POST ${path} failed: ${resp.status} ${text}`);
  }
}
