use std::process::Command;

use crate::filter::process_owned_by_current_user;

#[derive(Debug)]
pub enum KillError {
    NotOwned(u32),
    SignalFailed { pid: u32, message: String },
}

impl std::fmt::Display for KillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KillError::NotOwned(pid) => {
                write!(f, "refusing to kill pid {pid}: not owned by current user")
            }
            KillError::SignalFailed { pid, message } => {
                write!(f, "failed to kill pid {pid}: {message}")
            }
        }
    }
}

impl std::error::Error for KillError {}

pub fn kill_process(pid: u32, force: bool) -> Result<(), KillError> {
    if !process_owned_by_current_user(pid) {
        return Err(KillError::NotOwned(pid));
    }

    let signal = if force { 9 } else { 15 };
    send_signal(pid, signal)?;

    if !force && process_running(pid) {
        send_signal(pid, 9)?;
    }

    Ok(())
}

pub fn kill_pids(pids: &std::collections::HashSet<u32>, force: bool) -> Result<u32, KillError> {
    let mut killed = 0u32;
    let mut ordered: Vec<u32> = pids.iter().copied().collect();
    ordered.sort_unstable_by(|a, b| b.cmp(a));

    for pid in ordered {
        kill_process(pid, force)?;
        killed += 1;
    }

    Ok(killed)
}

fn send_signal(pid: u32, signal: i32) -> Result<(), KillError> {
    let status = Command::new("kill")
        .args(["-", &signal.to_string(), &pid.to_string()])
        .status()
        .map_err(|e| KillError::SignalFailed {
            pid,
            message: e.to_string(),
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(KillError::SignalFailed {
            pid,
            message: format!("kill exited with status {}", status.code().unwrap_or(-1)),
        })
    }
}

fn process_running(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

pub fn refresh_waybar(signal: u8) {
    let sig = format!("SIGRTMIN+{signal}");
    let _ = Command::new("pkill")
        .args(["-", &sig, "waybar"])
        .status();
}
