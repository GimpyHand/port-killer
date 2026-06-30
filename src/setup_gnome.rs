use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SERVER_ICON: &str = include_str!("../assets/icons/server-symbolic.svg");
const EXTENSION_UUID: &str = "port-killer@local";
const EXTENSION_JS: &str = include_str!("../gnome-extension/port-killer@local/extension.js");
const METADATA_TEMPLATE: &str = include_str!("../gnome-extension/port-killer@local/metadata.json");
const BINARY_PLACEHOLDER: &str = "PORT_KILLER_BINARY";

pub fn setup_gnome(install: bool, check_only: bool) -> Result<(), String> {
    let binary = crate::setup::resolve_binary_path()?;
    let ext_dir = extension_dir()?;

    if check_only {
        return print_gnome_diagnostics(&binary, &ext_dir);
    }

    if install {
        install_extension(&binary, &ext_dir)?;
        enable_extension()?;
        print_restart_instructions();
    } else {
        println!("GNOME Shell extension install:");
        println!("  port-killer setup gnome --install");
        println!();
        println!("Extension dir: {}", ext_dir.display());
        println!("Binary: {binary}");
    }

    Ok(())
}

pub fn is_gnome_session() -> bool {
    desktop_contains("gnome") || session_contains("gnome")
}

pub fn setup_desktop(install: bool, check_only: bool) -> Result<(), String> {
    if check_only {
        if is_gnome_session() {
            println!("Detected: GNOME (Ubuntu default top bar)\n");
            return setup_gnome(false, true);
        }
        if is_waybar_running() {
            println!("Detected: Waybar\n");
            return crate::setup::setup_waybar(false, None, true);
        }
        println!("Desktop: unknown");
        println!("GNOME session: {}", is_gnome_session());
        println!("Waybar running: {}", is_waybar_running());
        return Ok(());
    }

    if is_gnome_session() {
        setup_gnome(install, false)
    } else if is_waybar_running() {
        crate::setup::setup_waybar(install, None, false)
    } else if install {
        // Ubuntu default is GNOME — prefer gnome when unsure
        setup_gnome(true, false)
    } else {
        Err(
            "Could not detect desktop. Try:\n  port-killer setup gnome --install   (Ubuntu default)\n  port-killer setup waybar --install   (Hyprland/Sway/i3)"
                .to_string(),
        )
    }
}

fn extension_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home)
        .join(".local/share/gnome-shell/extensions")
        .join(EXTENSION_UUID))
}

fn install_extension(binary: &str, ext_dir: &PathBuf) -> Result<(), String> {
    fs::create_dir_all(ext_dir)
        .map_err(|e| format!("failed to create {}: {e}", ext_dir.display()))?;

    fs::write(ext_dir.join("extension.js"), EXTENSION_JS)
        .map_err(|e| format!("failed to write extension.js: {e}"))?;

    let icons_dir = ext_dir.join("icons");
    fs::create_dir_all(&icons_dir)
        .map_err(|e| format!("failed to create {}: {e}", icons_dir.display()))?;
    fs::write(icons_dir.join("server-symbolic.svg"), SERVER_ICON)
        .map_err(|e| format!("failed to write server icon: {e}"))?;

    let metadata = METADATA_TEMPLATE.replace(BINARY_PLACEHOLDER, binary);
    fs::write(ext_dir.join("metadata.json"), metadata)
        .map_err(|e| format!("failed to write metadata.json: {e}"))?;

    println!("Installed extension to {}", ext_dir.display());
    Ok(())
}

fn enable_extension() -> Result<(), String> {
    let output = Command::new("gnome-extensions")
        .args(["enable", EXTENSION_UUID])
        .output()
        .map_err(|e| format!("failed to run gnome-extensions: {e}"))?;

    if output.status.success() {
        println!("Enabled extension: {EXTENSION_UUID}");
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("already enabled") {
        println!("Extension already enabled.");
        return Ok(());
    }

    if stderr.contains("does not exist") || stderr.contains("doesn't exist") {
        println!("Extension files installed.");
        println!("GNOME must rescan extensions after install:");
        println!("  1. Restart shell (X11: Alt+F2 → r → Enter)");
        println!("  2. Then: gnome-extensions enable {EXTENSION_UUID}");
        return Ok(());
    }

    Err(format!(
        "gnome-extensions enable failed: {}",
        stderr.trim()
    ))
}

fn print_restart_instructions() {
    println!();
    println!("Restart GNOME Shell to show the icon:");
    println!("  X11:  Alt+F2 → type r → Enter");
    println!("  Wayland: log out and back in");
    println!();
    println!("Look for the server icon + count on the top bar (right side). Click to kill servers.");
}

fn print_gnome_diagnostics(binary: &str, ext_dir: &PathBuf) -> Result<(), String> {
    println!("port-killer GNOME diagnostics\n");
    println!("Session: {}", session_label());
    println!("Binary: {binary}");
    println!("Extension dir: {}", ext_dir.display());
    println!(
        "Extension installed: {}",
        ext_dir.join("extension.js").is_file()
    );

    if let Ok(output) = Command::new("gnome-extensions")
        .args(["info", EXTENSION_UUID])
        .output()
    {
        let info = String::from_utf8_lossy(&output.stdout);
        if info.contains("State: ACTIVE") {
            println!("Extension state: ACTIVE");
        } else if info.contains("State: ENABLED") {
            println!("Extension state: ENABLED (restart shell if no icon)");
        } else if output.status.success() {
            println!("Extension: installed but not active — run setup gnome --install");
        } else {
            println!("Extension: not enabled");
        }
    } else {
        println!("gnome-extensions: not available");
    }

    println!();
    if !ext_dir.join("extension.js").is_file() {
        println!("Fix: port-killer setup gnome --install");
    } else {
        println!("If icon missing: Alt+F2 → r (X11) or log out/in (Wayland)");
    }

    Ok(())
}

fn session_label() -> String {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    format!("{desktop} ({session})")
}

fn desktop_contains(needle: &str) -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|d| d.to_lowercase().contains(needle))
        .unwrap_or(false)
}

fn session_contains(needle: &str) -> bool {
    std::env::var("GNOME_DESKTOP_SESSION_ID")
        .is_ok()
        || std::env::var("DESKTOP_SESSION")
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
}

pub fn is_waybar_running() -> bool {
    Command::new("pgrep")
        .arg("waybar")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
