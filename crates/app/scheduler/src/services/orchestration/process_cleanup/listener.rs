//! Pure parsers behind the listening-socket lookup and the port-holder
//! identity check.
//!
//! Only sockets in the listening state name a port's holder: a client
//! connected to the port from elsewhere is not its holder, and a PID of 0 is
//! never a process this installation can own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::num::NonZeroU16;

// Why: Linux truncates a process `comm` to 15 bytes (TASK_COMM_LEN), so a
// longer executable name is only ever reported as its 15-byte prefix.
const COMM_NAME_LIMIT: usize = 15;

#[must_use]
pub fn parse_lsof_pids(stdout: &str) -> Vec<u32> {
    let mut pids = Vec::new();
    for pid in stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .filter(|pid| *pid != 0)
    {
        if !pids.contains(&pid) {
            pids.push(pid);
        }
    }
    pids
}

#[must_use]
pub fn parse_netstat_listeners(stdout: &str, port: NonZeroU16) -> Vec<u32> {
    let mut pids = Vec::new();
    for line in stdout.lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();
        let [proto, local, _foreign, state, pid] = columns.as_slice() else {
            continue;
        };
        if !proto.eq_ignore_ascii_case("TCP") || *state != "LISTENING" {
            continue;
        }
        let local_port = local
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok());
        if local_port != Some(port.get()) {
            continue;
        }
        if let Ok(pid) = pid.parse::<u32>()
            && pid != 0
            && !pids.contains(&pid)
        {
            pids.push(pid);
        }
    }
    pids
}

#[must_use]
pub fn executable_names_match(holder: &str, own: &str) -> bool {
    !holder.is_empty()
        && (holder == own || (holder.len() == COMM_NAME_LIMIT && own.starts_with(holder)))
}
