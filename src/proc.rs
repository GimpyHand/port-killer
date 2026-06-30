use std::collections::{HashMap, HashSet, VecDeque};

use crate::filter::process_owned_by_current_user;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcStat {
    ppid: u32,
    pgid: u32,
}

pub fn ppid(pid: u32) -> Option<u32> {
    read_stat(pid).map(|stat| stat.ppid)
}

pub fn pgid(pid: u32) -> Option<u32> {
    read_stat(pid).map(|stat| stat.pgid)
}

/// Immediate parent chain up to the first process not owned by the current user.
pub fn ancestors(pid: u32) -> Vec<u32> {
    let mut chain = Vec::new();
    let mut current = pid;

    while let Some(parent) = ppid(current) {
        if parent == 0 || !process_owned_by_current_user(parent) {
            break;
        }
        chain.push(parent);
        current = parent;
    }

    chain
}

pub fn direct_children(parent_pid: u32) -> Vec<u32> {
    let mut children = Vec::new();

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return children;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        if ppid(pid) == Some(parent_pid) {
            children.push(pid);
        }
    }

    children.sort_unstable();
    children
}

/// All user-owned descendants of `root`, excluding `root` itself.
pub fn user_owned_descendants(root: u32) -> HashSet<u32> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([root]);

    while let Some(pid) = queue.pop_front() {
        for child in direct_children(pid) {
            if !process_owned_by_current_user(child) || !seen.insert(child) {
                continue;
            }
            queue.push_back(child);
        }
    }

    seen
}

fn read_stat(pid: u32) -> Option<ProcStat> {
    let content = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = content.find(')')? + 2;
    let fields: Vec<&str> = content[rest..].split_whitespace().collect();
    if fields.len() < 3 {
        return None;
    }

    Some(ProcStat {
        ppid: fields[1].parse().ok()?,
        pgid: fields[2].parse().ok()?,
    })
}

/// Union listener PIDs that are parent/child of each other.
pub fn connected_listener_components(listener_pids: &HashSet<u32>) -> Vec<HashSet<u32>> {
    if listener_pids.is_empty() {
        return Vec::new();
    }

    let mut parent: HashMap<u32, u32> = HashMap::new();

    fn find(parent: &mut HashMap<u32, u32>, pid: u32) -> u32 {
        let mut current = pid;
        while let Some(&p) = parent.get(&current) {
            if p == current {
                break;
            }
            let root = find(parent, p);
            parent.insert(current, root);
            current = root;
        }
        parent.get(&pid).copied().unwrap_or(pid)
    }

    fn union(parent: &mut HashMap<u32, u32>, a: u32, b: u32) {
        let root_a = find(parent, a);
        let root_b = find(parent, b);
        if root_a != root_b {
            parent.insert(root_b, root_a);
        }
    }

    for &pid in listener_pids {
        find(&mut parent, pid);
        for ancestor in ancestors(pid) {
            if listener_pids.contains(&ancestor) {
                union(&mut parent, pid, ancestor);
            }
        }
    }

    let mut groups: HashMap<u32, HashSet<u32>> = HashMap::new();
    for &pid in listener_pids {
        groups.entry(find(&mut parent, pid)).or_default().insert(pid);
    }

    let mut components: Vec<HashSet<u32>> = groups.into_values().collect();
    components.sort_by_key(|group| *group.iter().min().unwrap_or(&0));
    components
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unions_parent_child_listener_pids() {
        let pids = HashSet::from([100, 300]);
        let groups = connected_listener_components(&pids);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn reads_current_process_ppid() {
        let self_pid = std::process::id();
        assert!(ppid(self_pid).is_some());
    }
}
