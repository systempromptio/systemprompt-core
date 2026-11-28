#!/usr/bin/env python3
"""Fail when a traced run reached for anything but loopback.

Input: the output of `strace -f -e trace=connect,sendto,sendmsg` around a
parity run. Every socket address in it must be a loopback address, and none
may be on the DNS port: a lookup means something tried to resolve a host. A
trace that holds no loopback connection at all is refused too, because that is
a trace of nothing.

The rule is documentation/future/harness/testing/01-strategy.md section 5.
"""

import ipaddress
import re
import sys

IPV4 = re.compile(r'sin_port=htons\((\d+)\), sin_addr=inet_addr\("([^"]+)"\)')
IPV6 = re.compile(r'sin6_port=htons\((\d+)\).*?inet_pton\(AF_INET6, "([^"]+)"')
DNS_PORT = 53


def is_loopback(text):
    try:
        address = ipaddress.ip_address(text)
    except ValueError:
        return False
    mapped = getattr(address, "ipv4_mapped", None)
    return (mapped or address).is_loopback


def inspect(lines):
    """(violations, loopback connection count) for a trace."""
    violations = []
    loopback = 0
    for number, line in enumerate(lines, 1):
        for port, address in IPV4.findall(line) + IPV6.findall(line):
            if not is_loopback(address):
                violations.append("line %d: a socket address outside loopback, %s port %s" % (number, address, port))
            elif int(port) == DNS_PORT:
                violations.append("line %d: a name lookup, %s port %s" % (number, address, port))
            else:
                loopback += 1
    return violations, loopback


def main(argv):
    if len(argv) != 2:
        print("usage: network_guard.py <strace output>", file=sys.stderr)
        return 2
    with open(argv[1], encoding="utf-8", errors="replace") as handle:
        violations, loopback = inspect(handle)
    if violations:
        print("network guard: the run reached outside loopback (%d):" % len(violations), file=sys.stderr)
        for violation in violations[:50]:
            print("  " + violation, file=sys.stderr)
        return 1
    if loopback == 0:
        print("network guard: the trace holds no loopback connection; nothing was traced", file=sys.stderr)
        return 1
    print("network guard: OK (%d socket addresses, all loopback, none on the DNS port)" % loopback)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
