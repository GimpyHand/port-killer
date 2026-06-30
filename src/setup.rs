use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const WAYBAR_CSS: &str = "
/* port-killer — Lucide server icon */
#custom-port-killer {
    background-image: url(\"icons/port-killer-server.svg\");
    background-size: 16px 16px;
    background-position: 2px center;
    background-repeat: no-repeat;
    padding-left: 22px;
}

#custom-port-killer.active {
    color: #a6e3a1;
}

#custom-port-killer.idle {
    color: #6c7086;
    opacity: 0.65;
}
";

pub fn setup_waybar(install: bool, config: Option<PathBuf>, check_only: bool) -> Result<(), String> {
    let binary = resolve_binary_path()?;

    if check_only {
        return print_diagnostics(&binary, config.as_deref());
    }

    let config_path = resolve_config_path(config)?;

    if install {
        let waybar_dir = config_path
            .parent()
            .ok_or_else(|| "invalid waybar config path".to_string())?;
        let style_path = waybar_dir.join("style.css");

        fs::create_dir_all(waybar_dir)
            .map_err(|e| format!("failed to create {}: {e}", waybar_dir.display()))?;

        patch_waybar_config(&config_path, &binary)?;
        patch_waybar_style(&style_path)?;
        install_waybar_icon(&waybar_dir)?;

        println!("Waybar config updated: {}", config_path.display());
        println!("Waybar style updated: {}", style_path.display());
        println!("Binary: {binary}");
        println!();
        println!("Restart Waybar:  pkill waybar; waybar &");
        println!("Or reload Hyprland:  hyprctl dispatch exec waybar");
    } else {
        let module = waybar_module(&binary);
        println!("Add to {}:\n", config_path.display());
        println!("{module}");
        println!();
        println!("Add \"custom/port-killer\" to modules-right or modules-left.");
        println!();
        println!("Or run: port-killer setup waybar --install");
    }

    Ok(())
}

pub fn resolve_binary_path() -> Result<String, String> {
    if let Ok(path) = which_binary("port-killer") {
        return Ok(path);
    }

    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let local = PathBuf::from(&home).join(".local/bin/port-killer");
    if local.is_file() {
        return Ok(local.display().to_string());
    }

    Err("port-killer not found on PATH. Run: ./install".to_string())
}

fn which_binary(name: &str) -> Result<String, String> {
    let output = Command::new("command")
        .args(["-v", name])
        .output()
        .map_err(|e| format!("failed to locate {name}: {e}"))?;

    if !output.status.success() {
        return Err(format!("{name} not found"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn resolve_config_path(override_path: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = override_path {
        return Ok(path);
    }

    if let Ok(path) = std::env::var("WAYBAR_CONFIG") {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    Ok(config_dir()?.join("waybar/config.jsonc"))
}

fn waybar_module(binary: &str) -> String {
    format!(
        r#"    "custom/port-killer": {{
        "format": "{{text}}",
        "return-type": "json",
        "interval": 5,
        "exec": "{binary} list",
        "exec-if": "test -x {binary}",
        "on-click": "sleep 0.1 && {binary} menu",
        "signal": 9,
        "tooltip": true
    }}"#
    )
}

fn print_diagnostics(binary: &str, config: Option<&Path>) -> Result<(), String> {
    let config_path = resolve_config_path(config.map(Path::to_path_buf))?;

    println!("port-killer Waybar diagnostics\n");
    println!("Binary: {binary}");
    println!(
        "Binary exists: {}",
        Path::new(binary).is_file()
    );
    println!("Config path: {}", config_path.display());
    println!(
        "Config exists: {}",
        config_path.exists()
    );

    if config_path.exists() {
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        println!(
            "Module in config: {}",
            content.contains("custom/port-killer")
        );
        println!(
            "Module in modules list: {}",
            content.contains("\"custom/port-killer\"")
        );
    } else {
        println!("Module in config: false");
        println!();
        println!("No Waybar config found. Run:");
        println!("  port-killer setup waybar --install");
    }

    let list = Command::new(binary).arg("list").output();
    match list {
        Ok(out) if out.status.success() => {
            let json = String::from_utf8_lossy(&out.stdout);
            println!("port-killer list: OK ({})", json.chars().take(60).collect::<String>());
        }
        Ok(out) => println!(
            "port-killer list: failed ({})",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        Err(e) => println!("port-killer list: {e}"),
    }

    if let Ok(output) = Command::new("pgrep").args(["-a", "waybar"]).output() {
        let ps = String::from_utf8_lossy(&output.stdout);
        if ps.trim().is_empty() {
            println!("Waybar running: no");
        } else {
            println!("Waybar running: yes");
            for line in ps.lines() {
                println!("  {line}");
            }
            if let Some(custom) = ps.split("-c").nth(1) {
                let custom_path = custom.split_whitespace().next().unwrap_or("").trim();
                if !custom_path.is_empty()
                    && custom_path != config_path.display().to_string().as_str()
                {
                    println!();
                    println!("Waybar uses a custom config: {custom_path}");
                    println!("Patch that file instead:");
                    println!("  port-killer setup waybar --install --config {custom_path}");
                }
            }
        }
    }

    println!();
    if !config_path.exists() || !fs::read_to_string(&config_path)
        .unwrap_or_default()
        .contains("custom/port-killer")
    {
        println!("Fix: port-killer setup waybar --install");
        if config_path.exists() {
            println!("Or merge manually if you use a custom config path:");
            println!("  port-killer setup waybar --config {}", config_path.display());
        }
    } else {
        println!("Config looks OK. Restart Waybar if the icon is still missing.");
    }

    Ok(())
}

fn patch_waybar_config(path: &Path, binary: &str) -> Result<(), String> {
    let module = waybar_module(binary);

    if path.exists() {
        let original =
            fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;

        if original.contains("custom/port-killer") {
            let updated = update_binary_paths(&original, binary);
            if updated != original {
                fs::write(path, updated)
                    .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
                println!("Updated port-killer paths in existing module.");
            } else {
                println!("Waybar module already configured.");
            }
            return Ok(());
        }

        let updated = insert_module_definition(&original, &module);
        let updated = insert_module_name(&updated, "modules-right");
        let updated = if updated.contains("\"custom/port-killer\"") {
            updated
        } else {
            insert_module_name(&updated, "modules-left")
        };

        fs::write(path, updated)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        return Ok(());
    }

    fs::write(path, default_waybar_config(&module))
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(())
}

fn update_binary_paths(config: &str, binary: &str) -> String {
    config
        .replace(
            "\"exec\": \"port-killer list\"",
            &format!("\"exec\": \"{binary} list\""),
        )
        .replace(
            "\"exec-if\": \"command -v port-killer\"",
            &format!("\"exec-if\": \"test -x {binary}\""),
        )
        .replace(
            "\"on-click\": \"sleep 0.1 && port-killer menu\"",
            &format!("\"on-click\": \"sleep 0.1 && {binary} menu\""),
        )
        .replace(
            "\"on-click\": \"sleep 0.1 && port-killer menu --gui\"",
            &format!("\"on-click\": \"sleep 0.1 && {binary} menu\""),
        )
}

fn default_waybar_config(module: &str) -> String {
    format!(
        r#"{{
{module},
    "modules-right": ["custom/port-killer"]
}}
"#
    )
}

fn insert_module_definition(config: &str, module: &str) -> String {
    let trimmed = config.trim_end();
    let Some(pos) = trimmed.rfind('}') else {
        return default_waybar_config(module);
    };

    let (before, after) = trimmed.split_at(pos);
    let before = before.trim_end();
    let needs_comma = !before.ends_with('{') && !before.ends_with(',');
    let comma = if needs_comma { "," } else { "" };

    format!("{before}{comma}\n{module}\n{after}")
}

fn insert_module_name(config: &str, key: &str) -> String {
    let needle = format!("\"{key}\"");
    let Some(key_pos) = config.find(&needle) else {
        return config.to_string();
    };

    let after_key = &config[key_pos..];
    let Some(bracket_offset) = after_key.find('[') else {
        return config.to_string();
    };

    let insert_pos = key_pos + bracket_offset + 1;
    let tail = &config[insert_pos..];
    if tail.contains("custom/port-killer") {
        return config.to_string();
    }

    let mut out = String::with_capacity(config.len() + 32);
    out.push_str(&config[..insert_pos]);
    out.push_str("\n        \"custom/port-killer\",");
    out.push_str(&config[insert_pos..]);
    out
}

fn install_waybar_icon(waybar_dir: &Path) -> Result<(), String> {
    let icons_dir = waybar_dir.join("icons");
    fs::create_dir_all(&icons_dir)
        .map_err(|e| format!("failed to create {}: {e}", icons_dir.display()))?;

    let icon_path = icons_dir.join(crate::icon::WAYBAR_ICON_FILE);
    fs::write(&icon_path, crate::icon::SERVER_ICON_SVG)
        .map_err(|e| format!("failed to write {}: {e}", icon_path.display()))?;
    Ok(())
}

fn patch_waybar_style(path: &Path) -> Result<(), String> {
    let content = if path.exists() {
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?
    } else {
        String::new()
    };

    if content.contains("port-killer-server.svg") {
        return Ok(());
    }

    if content.contains("#custom-port-killer") {
        let without_old: String = content
            .lines()
            .filter(|line| !line.contains("custom-port-killer"))
            .collect::<Vec<_>>()
            .join("\n");
        let updated = format!("{}\n{}", without_old.trim_end(), WAYBAR_CSS.trim_start());
        fs::write(path, updated)
            .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        return Ok(());
    }

    let updated = if content.is_empty() {
        WAYBAR_CSS.trim_start().to_string()
    } else {
        format!("{content}\n{WAYBAR_CSS}")
    };

    fs::write(path, updated).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(())
}

fn config_dir() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }

    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_module_definition() {
        let module = waybar_module("/usr/bin/port-killer");
        let input = r#"{
    "layer": "top"
}"#;
        let out = insert_module_definition(input, &module);
        assert!(out.contains("custom/port-killer"));
        assert!(out.contains("\"layer\": \"top\""));
    }

    #[test]
    fn inserts_into_modules_right() {
        let input = r#"{
    "modules-right": ["clock"]
}"#;
        let out = insert_module_name(input, "modules-right");
        assert!(out.contains("\"custom/port-killer\","));
    }
}
