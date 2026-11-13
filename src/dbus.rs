use std::process::Command;

pub const DBUS_DEST: &str = "org.gnome.Shell";
pub const DBUS_PATH: &str = "/org/gnome/Shell/Extensions/Wkill";
pub const DBUS_IFACE: &str = "org.gnome.Shell.Extensions.Wkill";

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub id: u64,
    pub pid: u32,
    pub focused: bool,
    pub client: String,
    pub wm_class: String,
    pub title: String,
}

fn run_gdbus(method: &str, args: &[String]) -> Result<String, String> {
    let full_method = format!("{DBUS_IFACE}.{method}");
    let out = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            DBUS_DEST,
            "--object-path",
            DBUS_PATH,
            "--method",
            &full_method,
        ])
        .args(args)
        .output()
        .map_err(|e| format!("cannot run gdbus: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if err.contains("ServiceUnknown")
            || err.contains("UnknownObject")
            || err.contains("UnknownMethod")
        {
            return Err("wkill bridge extension is not active. Run `wkill install`, then log out and back in once. If it still fails, check `gnome-extensions info wkill@celine1337` (State should be ACTIVE).".into());
        }
        return Err(err);
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn list_windows() -> Result<Vec<Window>, String> {
    let raw = run_gdbus("List", &[])?;
    parse_list_output(&raw)
}

pub fn kill_call(id: u64, mode: &str) -> Result<(), String> {
    run_gdbus("Kill", &[id.to_string(), format!("'{mode}'")])?;
    Ok(())
}

pub fn bridge_ok() -> Result<usize, String> {
    list_windows().map(|w| w.len())
}

/// Unescape a GVariant printed string body (without the surrounding quotes).
pub fn unescape_gvariant(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('\'') => out.push('\''),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Parse the `('...',)` output of `gdbus call ... List`.
pub fn parse_list_output(raw: &str) -> Result<Vec<Window>, String> {
    let raw = raw.trim();
    if !raw.starts_with("('") || !raw.ends_with("',)") {
        return Err(format!("unexpected List response: {raw}"));
    }
    let body = unescape_gvariant(&raw[2..raw.len() - 3]);
    let mut windows = Vec::new();
    for line in body.split('\n') {
        if line.is_empty() {
            continue;
        }
        windows.push(parse_window_line(line)?);
    }
    Ok(windows)
}

fn split_escaped(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('\\') => cur.push('\\'),
                Some('|') => cur.push('|'),
                Some('n') => cur.push('\n'),
                Some('r') => cur.push('\r'),
                Some(other) => {
                    cur.push('\\');
                    cur.push(other);
                }
                None => cur.push('\\'),
            },
            '|' => {
                fields.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

fn parse_window_line(line: &str) -> Result<Window, String> {
    let f = split_escaped(line);
    if f.len() != 6 {
        return Err(format!("malformed window record: {line}"));
    }
    Ok(Window {
        id: f[0]
            .parse()
            .map_err(|_| format!("bad window id: {}", f[0]))?,
        pid: f[1].parse().map_err(|_| format!("bad pid: {}", f[1]))?,
        focused: f[2] == "1",
        client: f[3].clone(),
        wm_class: f[4].clone(),
        title: f[5].clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gvariant_unescape() {
        assert_eq!(unescape_gvariant(r"a\nb\\c\'d"), "a\nb\\c'd");
        assert_eq!(unescape_gvariant("plain"), "plain");
    }

    #[test]
    fn parse_simple() {
        let raw = "('123|456|1|wayland|org.gnome.Nautilus|Files',)";
        let ws = parse_list_output(raw).unwrap();
        assert_eq!(ws.len(), 1);
        assert_eq!(
            ws[0],
            Window {
                id: 123,
                pid: 456,
                focused: true,
                client: "wayland".into(),
                wm_class: "org.gnome.Nautilus".into(),
                title: "Files".into(),
            }
        );
    }

    #[test]
    fn parse_escaped_pipe_and_newline_in_title() {
        let raw = r"('1|2|0|x11|term|a\\|b\\nc',)";
        let ws = parse_list_output(raw).unwrap();
        assert_eq!(ws[0].title, "a|b\nc");
    }

    #[test]
    fn parse_empty_list() {
        assert!(parse_list_output("('',)").unwrap().is_empty());
    }

    #[test]
    fn parse_multiple() {
        let raw = "('1|2|0|wayland|a|A\\n3|4|1|wayland|b|B',)";
        let ws = parse_list_output(raw).unwrap();
        assert_eq!(ws.len(), 2);
        assert!(ws[1].focused);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse_list_output("error").is_err());
        assert!(parse_list_output("('1|2|0|wayland|a',)").is_err());
    }
}
