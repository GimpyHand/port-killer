use crate::group::KillTarget;

impl KillTarget {
    /// Compact single-line row for terminal CLI (original layout).
    pub fn cli_line(&self) -> String {
        let ports = format_ports(self);
        let comm = primary_comm(self);
        let pid_summary = format_pid_summary(self);
        let address = format_addresses(self);
        format!("{ports:<18}  {comm:<14}  {pid_summary:<18}  {address}")
    }

    /// GNOME / Waybar dropdown menu title.
    pub fn panel_label(&self) -> String {
        let ports = format_ports(self);
        let comm = primary_comm(self);
        let project = primary_project(self);
        format!("{ports}  {project} ({comm})")
    }

    /// GNOME / Waybar dropdown menu subtitle.
    pub fn panel_detail(&self) -> String {
        let cwd = primary_cwd(self);
        let note = format_process_note(self);
        if note.is_empty() {
            cwd
        } else {
            format!("{cwd}  {note}")
        }
    }
}

fn format_ports(target: &KillTarget) -> String {
    let mut ports: Vec<String> = target
        .listeners
        .iter()
        .map(|l| format!(":{}", l.port))
        .collect();
    ports.sort();
    ports.dedup();
    ports.join(",")
}

fn format_pid_summary(target: &KillTarget) -> String {
    let all = target.all_pids();
    if all.len() == 1 {
        return format!("pid {}", all.iter().next().copied().unwrap_or(0));
    }
    let mut pids: Vec<u32> = all.into_iter().collect();
    pids.sort_unstable();
    format!(
        "pids {}",
        pids.iter()
            .map(|pid| pid.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn format_addresses(target: &KillTarget) -> String {
    let mut addresses: Vec<String> = target
        .listeners
        .iter()
        .map(|l| l.address.clone())
        .collect();
    addresses.sort();
    addresses.dedup();
    addresses.join(", ")
}

fn format_process_note(target: &KillTarget) -> String {
    let all = target.all_pids();
    let count = all.len();
    if count <= 1 {
        return String::new();
    }
    let workers = target.extra_pids.len();
    if workers > 0 {
        format!("+{workers} related")
    } else if target.listeners.len() > 1 {
        format!("{} ports", target.listeners.len())
    } else {
        format!("{count} procs")
    }
}

fn primary_comm(target: &KillTarget) -> String {
    target
        .listeners
        .first()
        .map(|l| l.comm.as_str())
        .unwrap_or("?")
        .to_string()
}

fn primary_project(target: &KillTarget) -> String {
    target
        .listeners
        .first()
        .map(|l| l.project.as_str())
        .unwrap_or("?")
        .to_string()
}

fn primary_cwd(target: &KillTarget) -> String {
    target
        .listeners
        .first()
        .map(|l| l.cwd.as_str())
        .unwrap_or("?")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listener::Listener;
    use std::collections::HashSet;

    fn sample() -> KillTarget {
        KillTarget {
            listeners: vec![Listener {
                port: 3100,
                pid: 1,
                comm: "bun".to_string(),
                address: "127.0.0.1".to_string(),
                project: "avsight".to_string(),
                cwd: "~/code/avsight".to_string(),
                launch_cmd: "bun run dev".to_string(),
            }],
            extra_pids: HashSet::new(),
        }
    }

    #[test]
    fn cli_line_is_single_row() {
        let line = sample().cli_line();
        assert!(!line.contains('\n'));
        assert!(line.contains(":3100"));
        assert!(line.contains("bun"));
    }

    #[test]
    fn panel_label_includes_project() {
        let label = sample().panel_label();
        assert!(label.contains("avsight"));
        assert!(label.contains(":3100"));
    }
}
