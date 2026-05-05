//! Linux `/proc` based PID/port lookups (avoids spawning `lsof`).
#![cfg(target_os = "linux")]

use std::os::unix::fs::MetadataExt;

pub(super) fn find_pid_by_port_proc(port: u16) -> Option<u32> {
    let Ok(tcp_content) = std::fs::read_to_string("/proc/net/tcp") else {
        return None;
    };

    let port_hex = format!("{port:X}");

    for line in tcp_content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        let local_addr = parts.get(1).copied().unwrap_or("");
        if !local_addr.contains(&port_hex) {
            continue;
        }

        let inode_str = parts.get(9).copied().unwrap_or("");
        let inode: u64 = match inode_str.parse() {
            Ok(i) => i,
            Err(e) => {
                tracing::trace!(error = %e, inode_str = %inode_str, "Skipping non-numeric inode entry");
                continue;
            },
        };

        return find_pid_by_inode(inode);
    }

    None
}

fn find_pid_by_inode(target_inode: u64) -> Option<u32> {
    let Ok(proc_dir) = std::fs::read_dir("/proc") else {
        return None;
    };

    for entry in proc_dir.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_dir() {
            continue;
        }

        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };

        if let Some(found_pid) = check_process_fd_for_inode(pid, target_inode) {
            return Some(found_pid);
        }
    }

    None
}

fn check_process_fd_for_inode(pid: u32, target_inode: u64) -> Option<u32> {
    let fd_path = format!("/proc/{pid}/fd");
    let Ok(fd_dir) = std::fs::read_dir(&fd_path) else {
        return None;
    };

    for entry in fd_dir.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.ino() == target_inode {
            return Some(pid);
        }
    }

    None
}

pub(super) fn get_port_by_pid_proc(pid: u32) -> Option<u16> {
    let Ok(tcp_content) = std::fs::read_to_string("/proc/net/tcp") else {
        return None;
    };

    let fd_path = format!("/proc/{pid}/fd");
    let Ok(fd_dir) = std::fs::read_dir(&fd_path) else {
        return None;
    };

    let fd_inodes: Vec<u64> = fd_dir
        .flatten()
        .filter_map(|entry| entry.metadata().ok().map(|m| m.ino()))
        .collect();

    for line in tcp_content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        let inode_str = parts.get(9).copied().unwrap_or("");
        let inode: u64 = match inode_str.parse() {
            Ok(i) => i,
            Err(e) => {
                tracing::trace!(error = %e, inode_str = %inode_str, "Skipping non-numeric inode entry");
                continue;
            },
        };

        if !fd_inodes.contains(&inode) {
            continue;
        }

        let local_addr = parts.get(1).copied().unwrap_or("");
        if let Some(port_str) = local_addr.split(':').next_back() {
            if let Ok(port) = u16::from_str_radix(port_str, 16) {
                return Some(port);
            }
        }
    }

    None
}
