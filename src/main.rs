mod group;
mod display;
mod doctor;
mod filter;
#[cfg(feature = "gui")]
mod gui;
mod icon;
mod kill;
mod listener;
mod menu;
mod proc;
mod project;
mod setup;
mod setup_gnome;
mod tui;

use std::io::IsTerminal;

use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};

#[derive(Parser)]
#[command(
    name = "port-killer",
    version,
    about = "List and kill dev server processes (CLI + GNOME / Waybar)",
    after_help = "QUICK START\n  port-killer                      interactive terminal picker\n  port-killer menu                 compact dropdown (Waybar click)\n  port-killer setup desktop --install   Ubuntu GNOME top bar dropdown\n  port-killer doctor"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive terminal picker (↑↓ move, space select, enter kill)
    #[command(visible_alias = "pick")]
    Interactive,
    /// Plain table of dev servers (for scripts and piping)
    Status,
    /// JSON output for Waybar (not for humans)
    List,
    /// JSON kill targets for GNOME / panel dropdown menus
    TargetsJson,
    /// Compact GTK dropdown (default), full window with --window, terminal with --tui
    Menu {
        /// Use terminal picker instead of graphical menu
        #[arg(long)]
        tui: bool,
        /// Open the full multi-select window instead of compact dropdown
        #[arg(long)]
        window: bool,
    },
    /// Kill a grouped target by index from `targets-json`
    KillGroup {
        index: usize,
    },
    /// Kill a process by PID (current user only)
    Kill {
        pid: u32,
        /// Send SIGKILL instead of SIGTERM
        #[arg(long)]
        force: bool,
    },
    /// Install or print Waybar configuration
    Setup {
        #[command(subcommand)]
        target: SetupTarget,
    },
    /// Check Ubuntu/Linux dependencies and port-killer health
    Doctor,
}

#[derive(Subcommand)]
enum SetupTarget {
    /// Auto-detect GNOME (Ubuntu default) or Waybar and install
    Desktop {
        #[arg(long)]
        install: bool,
        #[arg(long)]
        check: bool,
    },
    /// GNOME Shell top bar (Ubuntu 24 / 26 default)
    Gnome {
        #[arg(long)]
        install: bool,
        #[arg(long)]
        check: bool,
    },
    /// Waybar module (Hyprland, Sway, i3)
    Waybar {
        /// Patch config.jsonc and style.css automatically
        #[arg(long)]
        install: bool,
        /// Show what is configured and what is missing
        #[arg(long)]
        check: bool,
        /// Waybar config path (default: ~/.config/waybar/config.jsonc)
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
}

fn main() {
    if std::env::var_os(gui::gui_env_var()).is_some() {
        let window = std::env::var("PORT_KILLER_GUI_MODE").as_deref() == Ok("window");
        let result = if window {
            menu::run_window()
        } else {
            menu::run_dropdown()
        };
        if let Err(e) = result {
            eprintln!("port-killer: {e}");
            std::process::exit(1);
        }
        return;
    }

    let cli = Cli::parse();

    let result = match cli.command.unwrap_or_else(default_command) {
        Commands::Interactive => tui::run_interactive(),
        Commands::Status => cmd_status(),
        Commands::List => cmd_list(),
        Commands::TargetsJson => cmd_targets_json(),
        Commands::Menu { tui, window } => menu::run_menu(tui, window),
        Commands::KillGroup { index } => cmd_kill_group(index),
        Commands::Kill { pid, force } => cmd_kill(pid, force),
        Commands::Setup { target } => match target {
            SetupTarget::Desktop { install, check } => {
                setup_gnome::setup_desktop(install, check)
            }
            SetupTarget::Gnome { install, check } => setup_gnome::setup_gnome(install, check),
            SetupTarget::Waybar {
                install,
                check,
                config,
            } => setup::setup_waybar(install, config, check),
        },
        Commands::Doctor => doctor::run(),
    };

    if let Err(e) = result {
        eprintln!("port-killer: {e}");
        std::process::exit(1);
    }
}

fn default_command() -> Commands {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        Commands::Interactive
    } else {
        Commands::Status
    }
}

fn cmd_status() -> Result<(), String> {
    let listeners = listener::collect_listeners()?;

    if listeners.is_empty() {
        println!("No dev servers running.");
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Port").add_attribute(Attribute::Bold),
            Cell::new("Process").add_attribute(Attribute::Bold),
            Cell::new("PID").add_attribute(Attribute::Bold),
            Cell::new("Address").add_attribute(Attribute::Bold),
        ]);

    for entry in &listeners {
        table.add_row(vec![
            Cell::new(format!(":{}", entry.port)).fg(Color::Green),
            Cell::new(&entry.comm),
            Cell::new(entry.pid),
            Cell::new(&entry.address).fg(Color::DarkGrey),
        ]);
    }

    println!("{table}");
    println!("\nInteractive: port-killer");
    Ok(())
}

fn cmd_targets_json() -> Result<(), String> {
    let targets = group::build_kill_targets(listener::collect_listeners()?);
    let mut items = Vec::new();

    for (index, target) in targets.iter().enumerate() {
        items.push(format!(
            "{{\"index\":{index},\"label\":\"{}\",\"detail\":\"{}\"}}",
            json_escape(&target.panel_label()),
            json_escape(&target.panel_detail())
        ));
    }

    println!(
        "{{\"count\":{},\"targets\":[{}]}}",
        targets.len(),
        items.join(",")
    );
    Ok(())
}

fn cmd_kill_group(index: usize) -> Result<(), String> {
    let targets = group::build_kill_targets(listener::collect_listeners()?);
    let Some(target) = targets.get(index) else {
        return Err(format!("no kill target at index {index}"));
    };

    let pids = target.all_pids();
    let killed = kill::kill_pids(&pids, false).map_err(|e| e.to_string())?;
    kill::refresh_waybar(9);
    eprintln!("Killed {killed} process(es).");
    Ok(())
}

fn cmd_list() -> Result<(), String> {
    let listeners = listener::collect_listeners()?;
    println!("{}", waybar_json(&listeners));
    Ok(())
}

fn waybar_json(listeners: &[listener::Listener]) -> String {
    let (text, tooltip, class) = if listeners.is_empty() {
        (
            "0".to_string(),
            "No dev servers".to_string(),
            "idle".to_string(),
        )
    } else {
        let tooltip = listeners
            .iter()
            .map(listener::Listener::tooltip_line)
            .collect::<Vec<_>>()
            .join("\n");
        (
            listeners.len().to_string(),
            tooltip,
            "active".to_string(),
        )
    };

    format!(
        "{{\"text\":\"{}\",\"tooltip\":\"{}\",\"class\":\"{}\"}}",
        json_escape(&text),
        json_escape(&tooltip),
        json_escape(&class),
    )
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn cmd_kill(pid: u32, force: bool) -> Result<(), String> {
    kill::kill_process(pid, force).map_err(|e| e.to_string())?;
    kill::refresh_waybar(9);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_json_strings() {
        assert_eq!(json_escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }
}
