use std::path::{Path, PathBuf};

use crate::proc::ancestors;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchContext {
    pub cwd: PathBuf,
    pub project: String,
    pub launch_cmd: String,
}

pub fn resolve_launch_context(pid: u32) -> Option<LaunchContext> {
    for candidate in std::iter::once(pid).chain(ancestors(pid)) {
        let Some(cwd) = read_cwd(candidate) else {
            continue;
        };
        let launch_cmd = read_cmdline(candidate).unwrap_or_else(|| "?".to_string());
        let project = project_name_from_dir(&cwd);
        return Some(LaunchContext {
            cwd,
            project,
            launch_cmd,
        });
    }
    None
}

pub fn read_cwd(pid: u32) -> Option<PathBuf> {
    let link = format!("/proc/{pid}/cwd");
    std::fs::canonicalize(&link).ok()
}

pub fn read_cmdline(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if raw.is_empty() {
        return None;
    }

    let cmd = raw
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect::<Vec<_>>()
        .join(" ");

    if cmd.is_empty() {
        None
    } else {
        Some(shorten_cmd(&cmd))
    }
}

fn shorten_cmd(cmd: &str) -> String {
    const MAX: usize = 80;
    if cmd.chars().count() <= MAX {
        return cmd.to_string();
    }
    let trimmed: String = cmd.chars().take(MAX - 1).collect();
    format!("{trimmed}…")
}

pub fn project_name_from_dir(dir: &Path) -> String {
    if let Some(name) = read_json_name_field(&dir.join("package.json"), "name") {
        return name;
    }
    if let Some(name) = read_toml_package_name(&dir.join("Cargo.toml")) {
        return name;
    }
    if let Some(name) = read_json_name_field(&dir.join("composer.json"), "name") {
        return name;
    }

    dir.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("?")
        .to_string()
}

fn read_json_name_field(path: &Path, field: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let pattern = format!(r#""{field}"\s*:\s*"([^"]+)""#);
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(&content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

fn read_toml_package_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_package = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_package = false;
        }
        if !in_package {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix("name = ") {
            return Some(parse_toml_string(name));
        }
    }

    None
}

fn parse_toml_string(raw: &str) -> String {
    let trimmed = raw.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn display_path(path: &Path) -> String {
    if let Ok(home) = std::env::var("HOME") {
        let home = Path::new(&home);
        if let Ok(stripped) = path.strip_prefix(home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_json_name() {
        let dir = std::env::temp_dir().join("port-killer-test-package");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("package.json"),
            r#"{"name":"my-app","version":"1.0.0"}"#,
        )
        .unwrap();

        assert_eq!(project_name_from_dir(&dir), "my-app");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn falls_back_to_directory_name() {
        let dir = std::env::temp_dir().join("port-killer-demo-project");
        let _ = std::fs::create_dir_all(&dir);
        assert_eq!(project_name_from_dir(&dir), "port-killer-demo-project");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shortens_long_cmdline() {
        let long = "bun ".to_string() + &"x".repeat(100);
        let short = shorten_cmd(&long);
        assert!(short.ends_with('…'));
        assert!(short.chars().count() <= 80);
    }

    #[test]
    fn resolves_self_launch_context() {
        let ctx = resolve_launch_context(std::process::id()).expect("self context");
        assert!(ctx.cwd.is_dir());
        assert!(!ctx.project.is_empty());
        assert!(!ctx.launch_cmd.is_empty());
    }
}
