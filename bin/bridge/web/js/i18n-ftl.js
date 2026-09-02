/**
 * Fluent subset: message parsing plus `{ $var -> [key] a *[other] b }` select
 * resolution. Pluralisation is the English one/other split.
 */
export function parseFtl(src) {
  const out = new Map();
  let id = null;
  let buf = [];
  const flush = () => {
    if (id !== null) { out.set(id, buf.join("\n").trim()); }
    id = null; buf = [];
  };
  for (const raw of src.split(/\r?\n/)) {
    // Why: Fluent block values and select expressions continue on indented
    // lines; reading only the first line rendered a raw `{ $count ->` at the user.
    if (id !== null && /^\s+\S/.test(raw)) { buf.push(raw.trim()); continue; }
    const line = raw.trim();
    if (!line || line.startsWith("#")) { flush(); continue; }
    const eq = line.indexOf("=");
    if (eq <= 0) { flush(); continue; }
    flush();
    id = line.slice(0, eq).trim();
    const value = line.slice(eq + 1).trim();
    if (value) { buf.push(value); }
  }
  flush();
  return out;
}

// Why: matched by brace depth so a `{ $count }` placeholder inside a variant
// is not taken for the closing brace of the select.
export function resolveSelects(template, args) {
  const opener = /\{\s*\$([A-Za-z0-9_-]+)\s*->/;
  let str = template;
  let m;
  while ((m = opener.exec(str)) !== null) {
    let depth = 0, end = -1;
    for (let i = m.index; i < str.length; i++) {
      if (str[i] === "{") { depth++; }
      else if (str[i] === "}") { depth--; if (depth === 0) { end = i; break; } }
    }
    if (end < 0) { break; }
    const body = str.slice(m.index + m[0].length, end);
    const value = args ? args[m[1]] : undefined;
    str = str.slice(0, m.index) + pickVariant(body, value) + str.slice(end + 1);
  }
  return str;
}

function pickVariant(body, value) {
  const re = /(\*)?\[([^\]]+)\]/g;
  const variants = [];
  let match, prev = null;
  while ((match = re.exec(body)) !== null) {
    if (prev) { prev.text = body.slice(prev.textStart, match.index).trim(); }
    prev = { def: !!match[1], key: match[2], textStart: re.lastIndex };
    variants.push(prev);
  }
  if (prev) { prev.text = body.slice(prev.textStart).trim(); }
  if (!variants.length) { return ""; }
  const category = Number(value) === 1 ? "one" : "other";
  const chosen = variants.find((v) => v.key === String(value))
    || variants.find((v) => v.key === category)
    || variants.find((v) => v.def)
    || variants[0];
  return chosen ? chosen.text : "";
}
