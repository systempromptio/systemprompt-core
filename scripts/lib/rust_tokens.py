"""Rust source tokens for the text gates.

`tokenize` yields identifier, punctuation and string tokens with comments,
char literals and lifetimes dropped; `located` pairs each token with its
1-based line. `matching` and `matching_back` find a bracket's partner.
"""
import bisect


def tokenize(text):
    return _scan(text)[0]


def _scan(text):
    """Identifier, punctuation and string tokens of Rust source.

    Comments (nested block comments included), char literals and lifetimes are
    dropped. A string token is `("str", value)`; the value is the raw text
    between the quotes, enough for a `#[path]` argument.
    """
    tokens, starts = [], []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c.isspace():
            i += 1
        elif text.startswith("//", i):
            j = text.find("\n", i)
            i = n if j < 0 else j + 1
        elif text.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if text.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif text.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
        elif c in "rb" and raw_string_at(text, i):
            j = i + (2 if text.startswith("br", i) else 1)
            hashes = 0
            while text[j] == "#":
                hashes, j = hashes + 1, j + 1
            close = '"' + "#" * hashes
            end = text.find(close, j + 1)
            end = n if end < 0 else end
            tokens.append(("str", text[j + 1:end]))
            starts.append(i)
            i = end + len(close)
        elif c == '"' or (c in "bc" and text.startswith('"', i + 1)):
            j = i + 1 if c == '"' else i + 2
            start = j
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            tokens.append(("str", text[start:j]))
            starts.append(i)
            i = j + 1
        elif c == "'":
            if text.startswith("\\", i + 1):
                j = text.find("'", i + 3)
                i = n if j < 0 else j + 1
            elif i + 2 < n and text[i + 2] == "'":
                i += 3
            else:
                i += 1
        elif c.isalnum() or c == "_":
            j = i
            while j < n and (text[j].isalnum() or text[j] == "_"):
                j += 1
            word = text[i:j]
            if word == "r" and text.startswith("#", j) and j + 1 < n and (text[j + 1].isalpha() or text[j + 1] == "_"):
                k = j + 1
                while k < n and (text[k].isalnum() or text[k] == "_"):
                    k += 1
                word, j = "r#" + text[j + 1:k], k
            tokens.append(("id", word))
            starts.append(i)
            i = j
        else:
            tokens.append(("p", c))
            starts.append(i)
            i += 1
    return tokens, starts


def raw_string_at(text, i):
    j = i + (2 if text.startswith("br", i) else 1)
    if text.startswith("b", i) and not text.startswith("br", i):
        return False
    if i > 0 and (text[i - 1].isalnum() or text[i - 1] == "_"):
        return False
    while j < len(text) and text[j] == "#":
        j += 1
    return j < len(text) and text[j] == '"'


def matching(tokens, i, open_, close):
    depth = 0
    for j in range(i, len(tokens)):
        if tokens[j] == ("p", open_):
            depth += 1
        elif tokens[j] == ("p", close):
            depth -= 1
            if depth == 0:
                return j
    return len(tokens) - 1


def matching_back(tokens, i, open_, close):
    depth = 0
    for j in range(i, -1, -1):
        if tokens[j] == ("p", close):
            depth += 1
        elif tokens[j] == ("p", open_):
            depth -= 1
            if depth == 0:
                return j
    return 0


def located(text):
    """Tokens as `(kind, value, line)`; the line is where the token starts."""
    tokens, starts = _scan(text)
    newlines = [i for i, c in enumerate(text) if c == "\n"]
    return [(k, v, bisect.bisect_right(newlines, s) + 1) for (k, v), s in zip(tokens, starts)]



def rust_files(root, subdirs, exclude=()):
    """Every `*.rs` under `root/<subdir>` for each subdir, skipping `target*`
    directories and any path under an `exclude` prefix (root-relative)."""
    from pathlib import Path

    root = Path(root)
    found = []
    for sub in subdirs:
        base = root / sub
        if not base.exists():
            continue
        for path in sorted(base.rglob("*.rs")):
            rel = path.relative_to(root).as_posix()
            if any(part.startswith("target") for part in path.relative_to(root).parts[:-1]):
                continue
            if any(rel == e or rel.startswith(e.rstrip("/") + "/") for e in exclude):
                continue
            found.append((rel, path))
    return found
