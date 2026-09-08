#!/usr/bin/env bash
set -uo pipefail

# Machete rule: production crates compile in one shape.
#
# A test-only seam — a `test*` Cargo feature, a `cfg(feature = "test-…")`
# branch, a `test_api` module, an `unreachable_pub` suppression, or an
# env-driven test redirect — makes the released artifact differ from the one
# the suite exercises. Tests live in `crates/tests` and reach honest `pub`
# items; collaborators are injected through constructors or config.
#
# Checks:
#   1. Cargo features named `test*` in a `[features]` table. Dependencies such
#      as `test-log` sit in `[dependencies]`/`[dev-dependencies]` and are not
#      flagged — only the `[features]` table is tracked.
#   2. `cfg(feature = "test…")` / `cfg!(…)` / `cfg_attr(…)`, including `not(`.
#   3. `mod test_api`, `mod *_test_api`, `test_api::`, `#[path = "…_test_api.rs"]`.
#   4. `unreachable_pub` *suppressions* in production sources: an
#      `allow`/`expect` attribute in `.rs`, or an `= "allow"` lint setting in a
#      manifest. Enabling the lint (`unreachable_pub = "warn"`/`"deny"` in a
#      `[lints.rust]` or `[workspace.lints.rust]` table, as the root and the
#      bridge manifests both do) is the point of the rule, not a breach of it.
#   5. `SYSTEMPROMPT_TEST_` env redirects in production `.rs`.
#
# Scope: production sources and manifests in `crates/**`, `bin/bridge/**` and
# the `systemprompt` facade, tracked or not (`git ls-files -co`) — an untracked
# new file must not pass vacuously. `crates/tests/**` is out of scope.

MATCHES=""
while IFS= read -r file; do
    case "$file" in
        crates/tests/*) continue ;;
    esac

    case "$file" in
        *Cargo.toml)
            FOUND=$(awk '
                /^[[:space:]]*\[/ { in_features = ($0 ~ /^[[:space:]]*\[features\][[:space:]]*$/); next }
                in_features && /^[[:space:]]*"?test[A-Za-z0-9_-]*"?[[:space:]]*=/ {
                    print FILENAME ":" FNR ": test-only Cargo feature (" $0 ")"
                }
                /unreachable_pub[[:space:]]*=[[:space:]]*"allow"/ {
                    print FILENAME ":" FNR ": unreachable_pub suppressed (" $0 ")"
                }
            ' "$file")
            ;;
        *)
            FOUND=$(grep -nE 'cfg(_attr)?!?\((not\()?feature[[:space:]]*=[[:space:]]*"test|(^|[^A-Za-z0-9_])mod[[:space:]]+([a-z_]*_)?test_api|test_api::|#\[path[[:space:]]*=[[:space:]]*".*_test_api\.rs"|(allow|expect)\([^)]*unreachable_pub|SYSTEMPROMPT_TEST_' "$file" \
                | sed "s|^|$file:|; s|:\([0-9]*\):|:\1: test-only seam in production source: |")
            ;;
    esac
    [ -n "$FOUND" ] && MATCHES+="${FOUND}"$'\n'
done < <(git ls-files -co --exclude-standard \
    'crates/*.rs' 'crates/**/*.rs' 'bin/bridge/src/*.rs' 'bin/bridge/src/**/*.rs' \
    'systemprompt/src/*.rs' 'systemprompt/src/**/*.rs' \
    'crates/**/Cargo.toml' 'bin/bridge/Cargo.toml' 'bin/bridge/**/Cargo.toml' \
    'systemprompt/Cargo.toml' 'systemprompt/**/Cargo.toml' | sort -u)

if [ -z "$MATCHES" ]; then
    echo "lint-test-seams: OK (no test-only seams in production sources)"
    exit 0
fi

echo "lint-test-seams: production crates compile in one shape."
echo "Make the item \`pub\` in a \`pub mod\`, inject collaborators through constructors"
echo "or config, and move test conveniences into \`crates/tests/\`."
echo "See CLAUDE.md § Rust Standards."
echo ""
printf '%s' "$MATCHES"
exit 1
