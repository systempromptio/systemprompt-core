#!/usr/bin/env bash
# Install the Debian packages a CI job links against, bounded at every level.
#
# A runner's apt can stall indefinitely: a mirror that accepts the connection
# and then trickles or stops, a dpkg lock held by the image's own background
# apt run, or a debconf prompt with no terminal. Unbounded, any of those spends
# the whole job timeout in this step and the job is cancelled without a
# diagnosis. Here every apt command runs under `timeout`, apt itself is told to
# give up on a silent connection and on a held lock, and the whole sequence is
# retried a bounded number of times. The worst case is about ten and a half
# minutes (two more when `ripgrep` is requested); the workflow step's
# `timeout-minutes` is the last backstop.
#
# `ripgrep` is never taken from apt. The rg-based source gates depend on
# root-anchored `.gitignore` exceptions being honoured, and Ubuntu's ripgrep
# 14.1.0 mis-applies them: it silently skips files the gates must see. Asking
# for `ripgrep` here installs a pinned upstream release instead, verified
# against its SHA-256 and downloaded under the same bounded-retry discipline.
#
# Usage: scripts/ci-install-system-deps.sh <package>...
set -euo pipefail

if [ "$#" -eq 0 ]; then
    echo "usage: $0 <package>..." >&2
    exit 2
fi

ATTEMPTS=3
RIPGREP_VERSION=15.1.0
RIPGREP_MIN_MAJOR=15
DOWNLOAD_TIMEOUT_S=30

ripgrep_current() {
    command -v rg >/dev/null 2>&1 || return 1
    local major
    major=$(rg --version | head -n1 | awk '{ print $2 }' | cut -d. -f1)
    [ "${major:-0}" -ge "$RIPGREP_MIN_MAJOR" ]
}

install_ripgrep() {
    if ripgrep_current; then
        echo "ripgrep already current: $(rg --version | head -n1)"
        return 0
    fi
    local target sha256
    case "$(uname -m)" in
        x86_64)
            target=x86_64-unknown-linux-musl
            sha256=1c9297be4a084eea7ecaedf93eb03d058d6faae29bbc57ecdaf5063921491599
            ;;
        aarch64 | arm64)
            target=aarch64-unknown-linux-gnu
            sha256=2b661c6ef508e902f388e9098d9c4c5aca72c87b55922d94abdba830b4dc885e
            ;;
        *)
            echo "::error::no pinned ripgrep $RIPGREP_VERSION build for $(uname -m)"
            return 1
            ;;
    esac
    local name="ripgrep-$RIPGREP_VERSION-$target"
    local url="https://github.com/BurntSushi/ripgrep/releases/download/$RIPGREP_VERSION/$name.tar.gz"
    local work attempt
    work=$(mktemp -d)
    for attempt in $(seq 1 "$ATTEMPTS"); do
        echo "::group::ripgrep $RIPGREP_VERSION download attempt $attempt/$ATTEMPTS"
        if curl -fsSL --connect-timeout 15 --max-time "$DOWNLOAD_TIMEOUT_S" \
            -o "$work/$name.tar.gz" "$url" &&
            echo "$sha256  $work/$name.tar.gz" | sha256sum -c --quiet - &&
            tar -xzf "$work/$name.tar.gz" -C "$work" "$name/rg"; then
            sudo install -m 0755 "$work/$name/rg" /usr/local/bin/rg
            echo "::endgroup::"
            rm -rf "$work"
            hash -r
            echo "installed $(rg --version | head -n1)"
            ripgrep_current
            return
        fi
        echo "::endgroup::"
        echo "::warning::ripgrep download attempt $attempt/$ATTEMPTS failed or did not verify"
        rm -f "$work/$name.tar.gz"
        if [ "$attempt" -lt "$ATTEMPTS" ]; then
            sleep $((attempt * 10))
        fi
    done
    rm -rf "$work"
    echo "::error::could not install a verified ripgrep $RIPGREP_VERSION after $ATTEMPTS bounded attempts"
    return 1
}

packages=()
for package in "$@"; do
    if [ "$package" = ripgrep ]; then
        install_ripgrep
    else
        packages+=("$package")
    fi
done
if [ "${#packages[@]}" -eq 0 ]; then
    exit 0
fi
set -- "${packages[@]}"

installed=$(dpkg-query -W -f='${db:Status-Status}\n' "$@" 2>/dev/null | grep -cx installed || true)
if [ "${installed:-0}" -eq "$#" ]; then
    echo "already installed: $*"
    exit 0
fi

UPDATE_TIMEOUT_S=60
INSTALL_TIMEOUT_S=120

APT_OPTS=(
    -o Acquire::Retries=3
    -o Acquire::http::Timeout=30
    -o Acquire::https::Timeout=30
    -o DPkg::Lock::Timeout=60
    -o Dpkg::Use-Pty=0
    -o Dpkg::Options::=--force-confdef
    -o Dpkg::Options::=--force-confold
)

# man-db rebuilds its index on every package install; on hosted runners that
# trigger alone can take minutes and is never needed in CI.
sudo rm -f /var/lib/man-db/auto-update

apt_bounded() {
    local limit=$1
    shift
    sudo DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a \
        timeout --kill-after=10 "$limit" apt-get "${APT_OPTS[@]}" "$@"
}

for attempt in $(seq 1 "$ATTEMPTS"); do
    echo "::group::apt attempt $attempt/$ATTEMPTS: $*"
    if apt_bounded "$UPDATE_TIMEOUT_S" update &&
        apt_bounded "$INSTALL_TIMEOUT_S" install -y --no-install-recommends "$@"; then
        echo "::endgroup::"
        exit 0
    else
        status=$?
    fi
    echo "::endgroup::"
    echo "::warning::system dependency install attempt $attempt/$ATTEMPTS failed (exit $status; 124 = timed out)"
    if [ "$attempt" -lt "$ATTEMPTS" ]; then
        sleep $((attempt * 10))
    fi
done

echo "::error::could not install system dependencies after $ATTEMPTS bounded attempts: $*"
exit 1
