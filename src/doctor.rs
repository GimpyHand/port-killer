use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

struct Check {
    name: &'static str,
    status: CheckStatus,
    detail: String,
}

pub fn run() -> Result<(), String> {
    let checks = collect_checks();
    let mut failures = 0u32;
    let mut warnings = 0u32;

    println!("port-killer doctor\n");

    if let Some((id, version)) = read_os_release() {
        println!("OS: {id} {version}");
        match (id.as_str(), version.as_str()) {
            ("ubuntu", "24.04") | ("ubuntu", "26.04") => {
                println!("Ubuntu support: tested target");
            }
            ("ubuntu", _) => {
                println!("Ubuntu support: best-effort (tested on 24.04 and 26.04)");
            }
            _ => {
                println!("Ubuntu support: not targeted (may still work on Linux)");
            }
        }
        println!();
    }

    if crate::setup_gnome::is_gnome_session() {
        println!("Desktop: GNOME (Ubuntu default top bar)");
        println!("Panel setup: port-killer setup gnome --install");
    } else if crate::setup_gnome::is_waybar_running() {
        println!("Desktop: Waybar detected");
        println!("Bar setup: port-killer setup waybar --install");
    } else {
        println!("Desktop: port-killer setup desktop --install");
    }
    println!();

    for check in &checks {
        let icon = match check.status {
            CheckStatus::Ok => "ok",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "fail",
        };
        println!("[{icon}] {} — {}", check.name, check.detail);
        match check.status {
            CheckStatus::Fail => failures += 1,
            CheckStatus::Warn => warnings += 1,
            CheckStatus::Ok => {}
        }
    }

    if let Some(cmd) = ubuntu_apt_fix(&checks) {
        println!();
        println!("Ubuntu fix (24.04 / 26.04):");
        println!("  {cmd}");
        println!("Or: ./scripts/install-ubuntu-deps.sh");
    }

    println!();
    if failures > 0 {
        Err(format!("{failures} check(s) failed, {warnings} warning(s)"))
    } else {
        println!("Ready. Try: port-killer");
        Ok(())
    }
}

fn collect_checks() -> Vec<Check> {
    vec![
        check_command("ss (iproute2)", "ss", true, "sudo apt install iproute2"),
        check_command("cc (build-essential)", "cc", true, "sudo apt install build-essential"),
        check_command("cargo (rustup)", "cargo", false, "https://rustup.rs"),
        check_command("port-killer binary", "port-killer", false, "./install"),
        check_display(),
        check_gtk_libs(),
        check_command("waybar (optional)", "waybar", false, "sudo apt install waybar"),
        check_ss_listens(),
        check_port_killer_list(),
    ]
}

fn check_command(name: &'static str, cmd: &str, required: bool, fix: &'static str) -> Check {
    let ok = command_exists(cmd);
    let status = if ok {
        CheckStatus::Ok
    } else if required {
        CheckStatus::Fail
    } else {
        CheckStatus::Warn
    };
    let detail = if ok {
        which_path(cmd).unwrap_or_else(|| cmd.to_string())
    } else {
        format!("missing — {fix}")
    };
    Check { name, status, detail }
}

fn check_display() -> Check {
    let ok = crate::menu::display_available();
    Check {
        name: "display (GTK menu)",
        status: if ok { CheckStatus::Ok } else { CheckStatus::Warn },
        detail: if ok {
            "DISPLAY or WAYLAND_DISPLAY set".to_string()
        } else {
            "no graphical session — use port-killer menu --tui".to_string()
        },
    }
}

fn check_gtk_libs() -> Check {
    let gtk = Path::new("/usr/lib/x86_64-linux-gnu/libgtk-4.so.1").exists()
        || Path::new("/usr/lib/aarch64-linux-gnu/libgtk-4.so.1").exists()
        || which_path("pkg-config")
            .map(|p| {
                Command::new(p)
                    .args(["--exists", "gtk4"])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            })
            .unwrap_or(false);

    Check {
        name: "GTK 4 (built-in menu)",
        status: if gtk { CheckStatus::Ok } else { CheckStatus::Warn },
        detail: if gtk {
            "available".to_string()
        } else {
            "install dev libs: sudo apt install libgtk-4-dev libadwaita-1-dev".to_string()
        },
    }
}

fn check_ss_listens() -> Check {
    let output = Command::new("ss").args(["-tlnp"]).output();
    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let lines = text.lines().filter(|l| l.starts_with("LISTEN")).count();
            Check {
                name: "ss -tlnp",
                status: CheckStatus::Ok,
                detail: format!("{lines} LISTEN socket(s) visible"),
            }
        }
        Ok(out) => Check {
            name: "ss -tlnp",
            status: CheckStatus::Fail,
            detail: format!(
                "exit {}",
                out.status.code().unwrap_or(-1)
            ),
        },
        Err(e) => Check {
            name: "ss -tlnp",
            status: CheckStatus::Fail,
            detail: e.to_string(),
        },
    }
}

fn check_port_killer_list() -> Check {
    let Some(bin) = which_path("port-killer") else {
        return Check {
            name: "port-killer list",
            status: CheckStatus::Warn,
            detail: "skipped — binary not installed".to_string(),
        };
    };

    match Command::new(&bin).arg("list").output() {
        Ok(out) if out.status.success() => {
            let json = String::from_utf8_lossy(&out.stdout);
            if json.starts_with('{') && json.contains("\"text\"") {
                Check {
                    name: "port-killer list",
                    status: CheckStatus::Ok,
                    detail: json.chars().take(48).collect(),
                }
            } else {
                Check {
                    name: "port-killer list",
                    status: CheckStatus::Fail,
                    detail: "unexpected output".to_string(),
                }
            }
        }
        Ok(out) => Check {
            name: "port-killer list",
            status: CheckStatus::Fail,
            detail: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        },
        Err(e) => Check {
            name: "port-killer list",
            status: CheckStatus::Fail,
            detail: e.to_string(),
        },
    }
}

fn read_os_release() -> Option<(String, String)> {
    let content = fs::read_to_string("/etc/os-release").ok()?;
    let id = parse_os_field(&content, "ID")?;
    let version = parse_os_field(&content, "VERSION_ID").unwrap_or_else(|| "?".to_string());
    Some((id, version))
}

fn parse_os_field(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    for line in content.lines() {
        if let Some(value) = line.strip_prefix(&prefix) {
            return Some(value.trim_matches('"').to_string());
        }
    }
    None
}

fn command_exists(cmd: &str) -> bool {
    which_path(cmd).is_some()
}

fn which_path(cmd: &str) -> Option<String> {
    let mut search_dirs: Vec<std::path::PathBuf> = Vec::new();

    if let Some(paths) = std::env::var_os("PATH") {
        search_dirs.extend(std::env::split_paths(&paths));
    }

    for dir in ["/usr/local/bin", "/usr/bin", "/bin"] {
        search_dirs.push(std::path::PathBuf::from(dir));
    }

    if let Ok(home) = std::env::var("HOME") {
        search_dirs.push(std::path::PathBuf::from(format!("{home}/.local/bin")));
        search_dirs.push(std::path::PathBuf::from(format!("{home}/.cargo/bin")));
    }

    for dir in search_dirs {
        let path = dir.join(cmd);
        if path.is_file() {
            return Some(path.display().to_string());
        }
    }

    None
}

fn ubuntu_apt_fix(checks: &[Check]) -> Option<String> {
    let os = read_os_release()?;
    if os.0 != "ubuntu" {
        return None;
    }

    let needs_ss = checks.iter().any(|c| c.name.contains("ss") && c.status == CheckStatus::Fail);
    let needs_cc = checks.iter().any(|c| c.name.contains("cc") && c.status == CheckStatus::Fail);
    let needs_gtk = checks
        .iter()
        .any(|c| c.name.contains("GTK") && c.status != CheckStatus::Ok);
    let needs_waybar = checks
        .iter()
        .any(|c| c.name.contains("waybar") && c.status == CheckStatus::Warn);

    if !needs_ss && !needs_cc && !needs_gtk && !needs_waybar {
        return None;
    }

    let mut pkgs = Vec::new();
    if needs_cc {
        pkgs.push("build-essential");
    }
    if needs_ss {
        pkgs.push("iproute2");
    }
    if needs_gtk {
        pkgs.push("libgtk-4-dev");
        pkgs.push("libadwaita-1-dev");
    }
    if needs_waybar {
        pkgs.push("waybar");
    }

    Some(format!(
        "sudo apt update && sudo apt install -y {}",
        pkgs.join(" ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ubuntu_os_release() {
        let sample = r#"ID=ubuntu
VERSION_ID="24.04""#;
        assert_eq!(parse_os_field(sample, "ID"), Some("ubuntu".to_string()));
        assert_eq!(parse_os_field(sample, "VERSION_ID"), Some("24.04".to_string()));
    }
}
