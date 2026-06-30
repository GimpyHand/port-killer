use std::collections::{HashMap, HashSet};

use crate::listener::Listener;
use crate::proc::{connected_listener_components, user_owned_descendants};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillTarget {
    pub listeners: Vec<Listener>,
    /// User-owned worker/child processes not listening on a dev port.
    pub extra_pids: HashSet<u32>,
}

impl KillTarget {
    pub fn listener_pids(&self) -> HashSet<u32> {
        self.listeners.iter().map(|l| l.pid).collect()
    }

    pub fn all_pids(&self) -> HashSet<u32> {
        let mut pids = self.listener_pids();
        pids.extend(&self.extra_pids);
        pids
    }
}

pub fn build_kill_targets(listeners: Vec<Listener>) -> Vec<KillTarget> {
    if listeners.is_empty() {
        return Vec::new();
    }

    let listener_pids: HashSet<u32> = listeners.iter().map(|l| l.pid).collect();
    let by_pid: HashMap<u32, Vec<Listener>> = listeners.into_iter().fold(
        HashMap::new(),
        |mut map, listener| {
            map.entry(listener.pid).or_default().push(listener);
            map
        },
    );

    let components = connected_listener_components(&listener_pids);

    let mut targets = components
        .into_iter()
        .map(|component| {
            let mut group_listeners: Vec<Listener> = component
                .iter()
                .flat_map(|pid| by_pid.get(pid).into_iter().flatten().cloned())
                .collect();
            group_listeners.sort_by_key(|l| (l.port, l.pid));

            let extra_pids = collect_extra_pids(&component, &listener_pids);

            KillTarget {
                listeners: group_listeners,
                extra_pids,
            }
        })
        .collect::<Vec<_>>();

    targets.sort_by_key(|target| {
        target
            .listeners
            .first()
            .map(|l| l.port)
            .unwrap_or(u16::MAX)
    });

    targets
}

fn collect_extra_pids(component: &HashSet<u32>, all_listener_pids: &HashSet<u32>) -> HashSet<u32> {
    let mut extra = HashSet::new();

    for &pid in component {
        for descendant in user_owned_descendants(pid) {
            if all_listener_pids.contains(&descendant) {
                continue;
            }
            extra.insert(descendant);
        }
    }

    extra
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listener(port: u16, pid: u32, comm: &str) -> Listener {
        Listener {
            port,
            pid,
            comm: comm.to_string(),
            address: "127.0.0.1".to_string(),
            project: "demo".to_string(),
            cwd: "~/code/demo".to_string(),
            launch_cmd: "bun run dev".to_string(),
        }
    }

    #[test]
    fn groups_same_pid_ports_together() {
        let targets = build_kill_targets(vec![
            listener(3000, 100, "node"),
            listener(3001, 100, "node"),
        ]);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].listeners.len(), 2);
        assert_eq!(targets[0].listener_pids(), HashSet::from([100]));
    }

    #[test]
    fn keeps_unrelated_listeners_separate_without_proc_links() {
        let targets = build_kill_targets(vec![
            listener(3000, 100, "node"),
            listener(4000, 200, "bun"),
        ]);

        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn all_pids_includes_extras() {
        let mut target = KillTarget {
            listeners: vec![listener(3000, 100, "node")],
            extra_pids: HashSet::from([101, 102]),
        };
        assert_eq!(target.all_pids(), HashSet::from([100, 101, 102]));
        target.extra_pids.clear();
        assert_eq!(target.all_pids(), HashSet::from([100]));
    }
}