use std::collections::HashSet;

pub const MIN_PORT: u16 = 1024;
pub const MAX_PORT: u16 = 65535;

/// Common system ports excluded from dev-server view.
pub const DENYLIST: &[u16] = &[22, 53, 631, 5353, 9050, 11434];

pub fn is_dev_port(port: u16) -> bool {
    (MIN_PORT..=MAX_PORT).contains(&port) && !DENYLIST.contains(&port)
}

pub fn process_owned_by_current_user(pid: u32) -> bool {
    let status_path = format!("/proc/{pid}/status");
    let euid = geteuid();
    let uid = read_uid_field(&status_path, "Uid:");
    uid == euid
}

fn geteuid() -> u32 {
    read_uid_field("/proc/self/status", "Uid:")
}

fn read_uid_field(path: &str, prefix: &str) -> u32 {
    let Ok(content) = std::fs::read_to_string(path) else {
        return u32::MAX;
    };

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(prefix) {
            return rest
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(u32::MAX);
        }
    }

    u32::MAX
}

pub fn dedupe_listeners<T, F>(items: Vec<T>, key: F) -> Vec<T>
where
    F: Fn(&T) -> (u16, u32),
    T: Clone,
{
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key(item)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_port_range() {
        assert!(!is_dev_port(80));
        assert!(!is_dev_port(22));
        assert!(is_dev_port(3000));
        assert!(is_dev_port(5173));
        assert!(!is_dev_port(11434));
    }
}
