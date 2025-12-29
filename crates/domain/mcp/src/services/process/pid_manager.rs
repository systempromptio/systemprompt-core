use anyhow::Result;
use std::os::unix::fs::MetadataExt;
use std::process::Command;

fn find_pid_by_port_proc(port: u16) -> Option<u32> {
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

pub fn find_pid_by_port(port: u16) -> Result<Option<u32>> {
    if let Some(pid) = find_pid_by_port_proc(port) {
        return Ok(Some(pid));
    }

    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()?;

    if output.stdout.is_empty() {
        return Ok(None);
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u32>().ok())
        .map(Some)
        .map_or(Ok(None), Ok)
}

pub fn find_pids_by_name(process_name: &str) -> Result<Vec<u32>> {
    let output = Command::new("pgrep").args(["-f", process_name]).output()?;

    if output.stdout.is_empty() {
        return Ok(vec![]);
    }

    let pids = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect();

    Ok(pids)
}

fn get_port_by_pid_proc(pid: u32) -> Option<u16> {
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

pub fn get_port_by_pid(pid: u32) -> Result<Option<u16>> {
    if let Some(port) = get_port_by_pid_proc(pid) {
        return Ok(Some(port));
    }

    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-P", "-n"])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let port = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.contains("LISTEN"))
        .and_then(|line| {
            line.split_whitespace()
                .find(|part| part.contains(':'))
                .and_then(|part| part.split(':').next_back())
                .and_then(|port_part| port_part.parse::<u16>().ok())
        });

    Ok(port)
}

pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    let cmdline_path = format!("/proc/{pid}/cmdline");
    let Ok(content) = std::fs::read_to_string(&cmdline_path) else {
        return None;
    };

    content
        .split('\0')
        .next()
        .and_then(|cmd| std::path::Path::new(cmd).file_name())
        .map(|name| name.to_string_lossy().to_string())
}

pub fn find_process_on_port_with_name(port: u16, expected_name: &str) -> Result<Option<u32>> {
    let Some(pid) = find_pid_by_port(port)? else {
        return Ok(None);
    };

    let Some(actual_name) = get_process_name_by_pid(pid) else {
        return Ok(None);
    };

    if actual_name == expected_name {
        Ok(Some(pid))
    } else {
        Ok(None)
    }
}
