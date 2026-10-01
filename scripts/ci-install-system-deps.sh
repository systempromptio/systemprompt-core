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
# minutes; the workflow step's `timeout-minutes: 12` is the last backstop.
#
# Usage: scripts/ci-install-system-deps.sh <package>...
set -euo pipefail

if [ "$#" -eq 0 ]; then
    echo "usage: $0 <package>..." >&2
    exit 2
fi

installed=$(dpkg-query -W -f='${db:Status-Status}\n' "$@" 2>/dev/null | grep -cx installed || true)
if [ "${installed:-0}" -eq "$#" ]; then
    echo "already installed: $*"
    exit 0
fi

ATTEMPTS=3
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
