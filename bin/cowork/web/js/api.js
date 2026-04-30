const TOKEN = "__TOKEN__";

export function api(path, init) {
  const sep = path.includes("?") ? "&" : "?";
  return fetch(`${path}${sep}t=${encodeURIComponent(TOKEN)}`, init);
}

export async function post(path, body, onError) {
  try {
    const resp = await api(path, {
      method: "POST",
      headers: body ? { "Content-Type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!resp.ok && resp.status !== 204) {
      const text = await resp.text();
      onError?.(`request ${path} failed: ${resp.status} ${text}`);
    }
  } catch (e) {
    onError?.(`request ${path} error: ${e}`);
  }
}
