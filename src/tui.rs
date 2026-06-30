use std::collections::HashSet;
use std::io::IsTerminal;

use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};

use crate::group::{build_kill_targets, KillTarget};
use crate::kill::{kill_pids, refresh_waybar};
use crate::listener::{collect_listeners, Listener};

const WAYBAR_SIGNAL: u8 = 9;

pub fn is_interactive_terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

pub fn run_interactive() -> Result<(), String> {
    let listeners = collect_listeners()?;
    let targets = build_kill_targets(listeners.clone());

    if listeners.is_empty() {
        println!("No dev servers running.");
        println!("Ports scanned: TCP 1024–65535 (system ports excluded).");
        return Ok(());
    }

    let labels: Vec<String> = listeners.iter().map(Listener::compact_line).collect();

    let theme = ColorfulTheme::default();
    let selections = MultiSelect::with_theme(&theme)
        .with_prompt("↑↓ move  space select  enter confirm  esc cancel")
        .items(&labels)
        .interact()
        .map_err(|e| format!("menu failed: {e}"))?;

    if selections.is_empty() {
        println!("Cancelled.");
        return Ok(());
    }

    let pids = pids_for_selections(&listeners, &targets, &selections);
    let count = selections.len();

    let confirmed = Confirm::with_theme(&theme)
        .with_prompt(format!("Kill {count} server(s)?"))
        .default(false)
        .interact()
        .map_err(|e| format!("confirm failed: {e}"))?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    let killed = kill_pids(&pids, false).map_err(|e| e.to_string())?;
    refresh_waybar(WAYBAR_SIGNAL);
    println!("Killed {killed} process(es).");
    Ok(())
}

pub fn pids_for_selections(
    listeners: &[Listener],
    targets: &[KillTarget],
    selections: &[usize],
) -> HashSet<u32> {
    let mut pids = HashSet::new();
    for &index in selections {
        let Some(listener) = listeners.get(index) else {
            continue;
        };
        for target in targets {
            if target.listener_pids().contains(&listener.pid) {
                pids.extend(target.all_pids());
                break;
            }
        }
    }
    pids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_selection_to_related_group_pids() {
        let listeners = vec![Listener {
            port: 3000,
            pid: 999_991,
            comm: "node".to_string(),
            address: "127.0.0.1".to_string(),
            project: "demo".to_string(),
            cwd: "~/code/demo".to_string(),
            launch_cmd: "node".to_string(),
        }];
        let targets = vec![KillTarget {
            listeners: listeners.clone(),
            extra_pids: HashSet::from([999_992]),
        }];
        assert_eq!(
            pids_for_selections(&listeners, &targets, &[0]),
            HashSet::from([999_991, 999_992])
        );
    }
}
