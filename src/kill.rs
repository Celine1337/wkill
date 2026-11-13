use crate::dbus::{self, Window};
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

const CRITICAL_CLASSES: &[&str] = &["gnome-shell", "org.gnome.Shell", "mutter", "gdm"];

pub fn is_critical(w: &Window) -> bool {
    w.pid <= 1
        || CRITICAL_CLASSES
            .iter()
            .any(|c| w.wm_class.eq_ignore_ascii_case(c))
}

fn window_gone(id: u64) -> bool {
    match dbus::list_windows() {
        Ok(ws) => !ws.iter().any(|w| w.id == id),
        Err(_) => true,
    }
}

fn pid_gone(pid: u32) -> bool {
    !Path::new(&format!("/proc/{pid}")).exists()
}

fn wait_until(deadline_ms: u64, step_ms: u64, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if done() {
            return true;
        }
        if start.elapsed() >= Duration::from_millis(deadline_ms) {
            return false;
        }
        sleep(Duration::from_millis(step_ms));
    }
}

/// Close gracefully, then SIGTERM, then SIGKILL. Returns a human-readable outcome.
pub fn kill_window(w: &Window, force: bool) -> Result<String, String> {
    if is_critical(w) {
        return Err(format!(
            "refusing to kill system window '{}' (pid {})",
            w.title, w.pid
        ));
    }
    if force {
        dbus::kill_call(w.id, "kill")?;
        wait_until(1000, 100, || pid_gone(w.pid));
        return Ok(format!("killed '{}' (pid {}, SIGKILL)", w.title, w.pid));
    }
    dbus::kill_call(w.id, "close")?;
    if wait_until(800, 150, || window_gone(w.id)) {
        return Ok(format!("closed '{}' (pid {})", w.title, w.pid));
    }
    if w.pid > 1 {
        dbus::kill_call(w.id, "term").ok();
        if wait_until(1500, 150, || pid_gone(w.pid)) {
            return Ok(format!("terminated '{}' (pid {}, SIGTERM)", w.title, w.pid));
        }
        dbus::kill_call(w.id, "kill").ok();
        wait_until(1000, 100, || pid_gone(w.pid));
    }
    if pid_gone(w.pid) || window_gone(w.id) {
        Ok(format!("killed '{}' (pid {}, SIGKILL)", w.title, w.pid))
    } else {
        Err(format!("'{}' (pid {}) survived SIGKILL", w.title, w.pid))
    }
}

pub fn focused() -> Result<Window, String> {
    let ws = dbus::list_windows()?;
    ws.into_iter()
        .find(|w| w.focused)
        .ok_or_else(|| "no focused window (are you in the overview?)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(wm_class: &str, pid: u32) -> Window {
        Window {
            id: 1,
            pid,
            focused: true,
            client: "wayland".into(),
            wm_class: wm_class.into(),
            title: "t".into(),
        }
    }

    #[test]
    fn critical_guard() {
        assert!(is_critical(&win("gnome-shell", 123)));
        assert!(is_critical(&win("org.gnome.Shell", 123)));
        assert!(is_critical(&win("firefox", 1)));
        assert!(!is_critical(&win("firefox", 4242)));
    }
}
