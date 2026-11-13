use std::process::Command;

const MK: &str = "org.gnome.settings-daemon.plugins.media-keys";
const SLOT_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const DEFAULT_BINDING: &str = "<Ctrl><Alt>K";

fn gsettings(args: &[&str]) -> Result<String, String> {
    let out = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| format!("cannot run gsettings: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        crate::dbus::unescape_gvariant(&s[1..s.len() - 1])
    } else {
        s.to_string()
    }
}

/// Parse a GVariant string array like `['/a/', '/b/']` or `@as []`.
pub fn parse_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.starts_with('@') || s == "[]" {
        return Vec::new();
    }
    let inner = s.trim_start_matches('[').trim_end_matches(']');
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(',').map(unquote).collect()
}

pub fn format_array(paths: &[String]) -> String {
    let items: Vec<String> = paths.iter().map(|p| format!("'{p}'")).collect();
    format!("[{}]", items.join(", "))
}

pub fn slot_path(index: u32) -> String {
    format!("/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom{index}/")
}

pub fn list_slots() -> Result<Vec<String>, String> {
    Ok(parse_array(&gsettings(&["get", MK, "custom-keybindings"])?))
}

pub struct Slot {
    pub path: String,
    pub name: String,
    pub binding: String,
    pub command: String,
}

pub fn read_slot(path: &str) -> Result<Slot, String> {
    let schema = format!("{SLOT_SCHEMA}:{path}");
    Ok(Slot {
        path: path.to_string(),
        name: unquote(&gsettings(&["get", &schema, "name"])?),
        binding: unquote(&gsettings(&["get", &schema, "binding"])?),
        command: unquote(&gsettings(&["get", &schema, "command"])?),
    })
}

fn write_slot(path: &str, name: &str, binding: &str, command: &str) -> Result<(), String> {
    let schema = format!("{SLOT_SCHEMA}:{path}");
    gsettings(&["set", &schema, "name", &format!("'{name}'")])?;
    gsettings(&["set", &schema, "binding", &format!("'{binding}'")])?;
    gsettings(&["set", &schema, "command", &format!("'{command}'")])?;
    Ok(())
}

fn is_xkill_slot(s: &Slot) -> bool {
    s.command == "xkill" || s.command.ends_with("/xkill") || s.name.eq_ignore_ascii_case("xkill")
}

fn is_wkill_slot(s: &Slot, command: &str) -> bool {
    s.name == "wkill" || s.command == command || s.command.ends_with("/wkill kill")
}

/// Register the hotkey. Reuses an existing xkill slot (keeping its binding),
/// then an existing wkill slot, then the first free customN slot.
pub fn register(command: &str, preferred_binding: Option<&str>) -> Result<String, String> {
    let mut slots = list_slots()?;
    let mut reuse: Option<(String, String)> = None;
    let mut existing_wkill: Option<String> = None;
    for path in &slots {
        let s = read_slot(path)?;
        if is_xkill_slot(&s) && reuse.is_none() {
            reuse = Some((path.clone(), s.binding.clone()));
        }
        if is_wkill_slot(&s, command) && existing_wkill.is_none() {
            existing_wkill = Some(path.clone());
        }
    }
    let (path, binding) = if let Some((p, b)) = reuse {
        (p, b)
    } else if let Some(p) = existing_wkill {
        let b = read_slot(&p)?.binding;
        (
            p,
            if b.is_empty() {
                DEFAULT_BINDING.to_string()
            } else {
                b
            },
        )
    } else {
        let mut idx = 0u32;
        while slots.contains(&slot_path(idx)) {
            idx += 1;
        }
        let p = slot_path(idx);
        slots.push(p.clone());
        gsettings(&["set", MK, "custom-keybindings", &format_array(&slots)])?;
        (p, preferred_binding.unwrap_or(DEFAULT_BINDING).to_string())
    };
    write_slot(&path, "wkill", &binding, command)?;
    Ok(binding)
}

pub fn find_wkill_slot(command: &str) -> Result<Option<Slot>, String> {
    for path in list_slots()? {
        let s = read_slot(&path)?;
        if is_wkill_slot(&s, command) {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub fn set_binding(command: &str, binding: &str) -> Result<(), String> {
    let slot =
        find_wkill_slot(command)?.ok_or("no wkill hotkey registered; run `wkill install` first")?;
    write_slot(&slot.path, "wkill", binding, command)
}

pub fn unregister(command: &str) -> Result<bool, String> {
    let mut slots = list_slots()?;
    let mut removed = false;
    let mut keep = Vec::new();
    for path in slots.drain(..) {
        let s = read_slot(&path)?;
        if is_wkill_slot(&s, command) {
            let schema = format!("{SLOT_SCHEMA}:{path}");
            for key in ["name", "binding", "command"] {
                gsettings(&["reset", &schema, key])?;
            }
            removed = true;
        } else {
            keep.push(path);
        }
    }
    if removed {
        gsettings(&["set", MK, "custom-keybindings", &format_array(&keep)])?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_roundtrip() {
        let a = vec!["/x/custom0/".to_string(), "/x/custom1/".to_string()];
        assert_eq!(parse_array(&format_array(&a)), a);
    }

    #[test]
    fn array_empty_forms() {
        assert!(parse_array("@as []").is_empty());
        assert!(parse_array("[]").is_empty());
        assert!(parse_array("['/a/']").eq(&vec!["/a/".to_string()]));
    }

    #[test]
    fn unquote_basic() {
        assert_eq!(unquote("'<Ctrl><Alt>K'"), "<Ctrl><Alt>K");
        assert_eq!(unquote("plain"), "plain");
    }

    #[test]
    fn slot_detection() {
        let x = Slot {
            path: "/p/".into(),
            name: "xkill".into(),
            binding: "<Shift><Control>x".into(),
            command: "xkill".into(),
        };
        assert!(is_xkill_slot(&x));
        let w = Slot {
            path: "/p/".into(),
            name: "wkill".into(),
            binding: String::new(),
            command: "/home/u/.local/bin/wkill kill".into(),
        };
        assert!(is_wkill_slot(&w, "/home/u/.local/bin/wkill kill"));
        assert!(!is_xkill_slot(&w));
    }
}
