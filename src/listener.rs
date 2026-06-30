use std::process::Command;

use regex::Regex;

use crate::filter::{self, is_dev_port};
use crate::project::{display_path, resolve_launch_context};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub port: u16,
    pub pid: u32,
    pub comm: String,
    pub address: String,
    pub project: String,
    pub cwd: String,
    pub launch_cmd: String,
}

impl Listener {
    pub fn compact_line(&self) -> String {
        format!(
            ":{:<5}  {:<14}  pid {:<6}  {}",
            self.port, self.comm, self.pid, self.address
        )
    }

    pub fn tooltip_line(&self) -> String {
        format!(
            ":{} · {} · {} · pid {}",
            self.port, self.project, self.comm, self.pid
        )
    }
}

fn enrich_listener(port: u16, pid: u32, comm: String, address: String) -> Listener {
    let (project, cwd, launch_cmd) = match resolve_launch_context(pid) {
        Some(ctx) => (
            ctx.project,
            display_path(&ctx.cwd),
            ctx.launch_cmd,
        ),
        None => ("?".to_string(), "?".to_string(), "?".to_string()),
    };

    Listener {
        port,
        pid,
        comm,
        address,
        project,
        cwd,
        launch_cmd,
    }
}

pub fn collect_listeners() -> Result<Vec<Listener>, String> {
    let output = Command::new("ss")
        .args(["-tlnp"])
        .output()
        .map_err(|e| format!("failed to run ss: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ss exited with status {}",
            output.status.code().unwrap_or(-1)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ss_output(&stdout))
}

fn parse_ss_output(stdout: &str) -> Vec<Listener> {
    let line_re = Regex::new(
        r#"^LISTEN\s+\d+\s+\d+\s+(\S+)\s+\S+\s+(?:users:)?\(\(\"([^"]+)\",pid=(\d+)"#,
    )
    .expect("valid ss line regex");

    let mut listeners = Vec::new();

    for line in stdout.lines().skip(1) {
        let Some(caps) = line_re.captures(line) else {
            continue;
        };

        let local = caps.get(1).map(|m| m.as_str()).unwrap_or("*");
        let comm = caps.get(2).map(|m| m.as_str()).unwrap_or("?").to_string();
        let pid: u32 = caps
            .get(3)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);

        if pid == 0 {
            continue;
        }

        let Some((address, port)) = parse_local_address_port(local) else {
            continue;
        };

        if !is_dev_port(port) {
            continue;
        }

        if !filter::process_owned_by_current_user(pid) {
            continue;
        }

        listeners.push(enrich_listener(port, pid, comm, address));
    }

    listeners.sort_by(|a, b| {
        a.port
            .cmp(&b.port)
            .then_with(|| a.comm.cmp(&b.comm))
            .then_with(|| a.pid.cmp(&b.pid))
    });

    filter::dedupe_listeners(listeners, |l| (l.port, l.pid))
}

fn parse_local_address_port(field: &str) -> Option<(String, u16)> {
    if field == "*" {
        return None;
    }

    if let Some(rest) = field.strip_prefix('[') {
        let end = rest.find("]:")?;
        let address = format!("[{}]", &rest[..end]);
        let port: u16 = rest[end + 2..].parse().ok()?;
        return Some((address, port));
    }

    let (address, port_str) = field.rsplit_once(':')?;
    let port: u16 = port_str.parse().ok()?;
    Some((address.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ipv4_and_ipv6_local() {
        assert_eq!(
            parse_local_address_port("127.0.0.1:3000"),
            Some(("127.0.0.1".to_string(), 3000))
        );
        assert_eq!(
            parse_local_address_port("[::1]:5173"),
            Some(("[::1]".to_string(), 5173))
        );
        assert_eq!(
            parse_local_address_port("127.0.0.53%lo:53"),
            Some(("127.0.0.53%lo".to_string(), 53))
        );
    }

    #[test]
    fn parses_ubuntu_24_ss_lines() {
        let line_re = Regex::new(
            r#"^LISTEN\s+\d+\s+\d+\s+(\S+)\s+\S+\s+(?:users:)?\(\(\"([^"]+)\",pid=(\d+)"#,
        )
        .unwrap();

        let samples = [
            "LISTEN 0      512          0.0.0.0:3101       0.0.0.0:*    users:((\"node\",pid=74937,fd=16))    ",
            "LISTEN 0      512        127.0.0.1:4096       0.0.0.0:*    users:((\"opencode\",pid=9800,fd=17)) ",
            "LISTEN 0      511        127.0.0.1:27123      0.0.0.0:*    users:((\"obsidian\",pid=34086,fd=39))",
            "LISTEN 0      4096   127.0.0.53%lo:53         0.0.0.0:*    ",
        ];

        for (index, line) in samples.iter().enumerate() {
            let matched = line_re.is_match(line);
            if index < 3 {
                assert!(matched, "expected match: {line}");
            } else {
                assert!(!matched, "system socket without pid should not match: {line}");
            }
        }
    }

    #[test]
    fn parses_ss_sample() {
        let line_re = Regex::new(
            r#"^LISTEN\s+\d+\s+\d+\s+(\S+)\s+\S+\s+(?:users:)?\(\(\"([^"]+)\",pid=(\d+)"#,
        )
        .unwrap();
        assert!(line_re.is_match(
            "LISTEN 0      511    127.0.0.1:3000      0.0.0.0:*    users:((\"node\",pid=12345,fd=22))"
        ));
    }
}
