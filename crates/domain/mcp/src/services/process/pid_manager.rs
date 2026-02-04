use anyhow::Result;
use std::process::Command;

#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub fn find_pid_by_port(port: u16) -> Result<Option<u32>> {
    if let Some(pid) = find_pid_by_port_proc(port) {
        return Ok(Some(pid));
    }

    find_pid_by_port_lsof(port)
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn find_pid_by_port(port: u16) -> Result<Option<u32>> {
    find_pid_by_port_lsof(port)
}

#[cfg(unix)]
fn find_pid_by_port_lsof(port: u16) -> Result<Option<u32>> {
    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()?;

    if output.stdout.is_empty() {
        return Ok(None);
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u32>().ok()))
}

#[cfg(windows)]
pub fn find_pid_by_port(port: u16) -> Result<Option<u32>> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_pattern = format!(":{port} ");
    let port_pattern_tab = format!(":{port}\t");

    for line in stdout.lines() {
        if line.contains(&port_pattern) || line.contains(&port_pattern_tab) {
            if let Some(pid_str) = line.split_whitespace().last() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return Ok(Some(pid));
                }
            }
        }
    }

    Ok(None)
}

#[cfg(unix)]
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

#[cfg(windows)]
pub fn find_pids_by_name(process_name: &str) -> Result<Vec<u32>> {
    let output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pids = Vec::new();

    for line in stdout.lines() {
        if line.to_lowercase().contains(&process_name.to_lowercase()) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                if let Ok(pid) = parts[1].trim_matches('"').parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
    }

    Ok(pids)
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
pub fn get_port_by_pid(pid: u32) -> Result<Option<u16>> {
    if let Some(port) = get_port_by_pid_proc(pid) {
        return Ok(Some(port));
    }

    get_port_by_pid_lsof(pid)
}

#[cfg(all(unix, not(target_os = "linux")))]
pub fn get_port_by_pid(pid: u32) -> Result<Option<u16>> {
    get_port_by_pid_lsof(pid)
}

#[cfg(unix)]
fn get_port_by_pid_lsof(pid: u32) -> Result<Option<u16>> {
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

#[cfg(windows)]
pub fn get_port_by_pid(pid: u32) -> Result<Option<u16>> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid_str = pid.to_string();

    for line in stdout.lines() {
        if line.contains("LISTENING") && line.ends_with(&pid_str) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Some(port_str) = parts[1].split(':').last() {
                    if let Ok(port) = port_str.parse::<u16>() {
                        return Ok(Some(port));
                    }
                }
            }
        }
    }

    Ok(None)
}

#[cfg(unix)]
pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(windows)]
pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim();

    if line.is_empty() || line.contains("INFO: No tasks") {
        return None;
    }

    let parts: Vec<&str> = line.split(',').collect();
    if parts.is_empty() {
        return None;
    }

    Some(parts[0].trim_matches('"').to_string())
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
