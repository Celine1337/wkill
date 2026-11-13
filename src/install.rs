use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const UUID: &str = "wkill@celine1337";
const EXT_JS: &str = include_str!("../extension/extension.js");
const EXT_META: &str = include_str!("../extension/metadata.json");

pub fn extensions_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set")?;
    Ok(Path::new(&home)
        .join(".local/share/gnome-shell/extensions")
        .join(UUID))
}

pub fn installed_bin() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set")?;
    Ok(Path::new(&home).join(".local/bin/wkill"))
}

fn gnome_extensions(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gnome-extensions")
        .args(args)
        .output()
        .map_err(|e| format!("cannot run gnome-extensions: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    Ok(stdout)
}

pub fn extension_state() -> String {
    match gnome_extensions(&["info", UUID]) {
        Ok(info) => info
            .lines()
            .find(|l| l.trim_start().starts_with("State:"))
            .map(|l| l.trim().trim_start_matches("State:").trim().to_string())
            .unwrap_or_else(|| "UNKNOWN".into()),
        Err(_) => {
            if extensions_dir().map(|d| d.exists()).unwrap_or(false) {
                "INSTALLED (activates after re-login)".into()
            } else {
                "NOT INSTALLED".into()
            }
        }
    }
}

fn shell_gsettings(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| format!("cannot run gsettings: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The running shell only knows extensions scanned at startup, so a freshly
/// installed one must be queued in enabled-extensions for the next login.
fn ensure_enabled_at_login() -> Result<(), String> {
    let mut list = crate::hotkey::parse_array(&shell_gsettings(&[
        "get",
        "org.gnome.shell",
        "enabled-extensions",
    ])?);
    if !list.iter().any(|u| u == UUID) {
        list.push(UUID.to_string());
        shell_gsettings(&[
            "set",
            "org.gnome.shell",
            "enabled-extensions",
            &crate::hotkey::format_array(&list),
        ])?;
    }
    Ok(())
}

fn remove_enabled_at_login() -> Result<(), String> {
    let list = crate::hotkey::parse_array(&shell_gsettings(&[
        "get",
        "org.gnome.shell",
        "enabled-extensions",
    ])?);
    let keep: Vec<String> = list.into_iter().filter(|u| u != UUID).collect();
    shell_gsettings(&[
        "set",
        "org.gnome.shell",
        "enabled-extensions",
        &crate::hotkey::format_array(&keep),
    ])?;
    Ok(())
}

pub fn install_extension() -> Result<(), String> {
    let dir = extensions_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    fs::write(dir.join("extension.js"), EXT_JS).map_err(|e| e.to_string())?;
    fs::write(dir.join("metadata.json"), EXT_META).map_err(|e| e.to_string())?;
    if gnome_extensions(&["enable", UUID]).is_err() {
        ensure_enabled_at_login()?;
    }
    Ok(())
}

pub fn uninstall_extension() -> Result<(), String> {
    let _ = gnome_extensions(&["disable", UUID]);
    remove_enabled_at_login()?;
    let dir = extensions_dir()?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Copy the running binary to ~/.local/bin/wkill unless it is already there.
pub fn install_binary() -> Result<PathBuf, String> {
    let target = installed_bin()?;
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    if current == target {
        return Ok(target);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(&current, &target).map_err(|e| format!("cannot copy binary: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(target)
}

pub fn gnome_version() -> String {
    Command::new("gnome-shell")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

pub fn session_type() -> String {
    std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".into())
}
