use std::process::Command;

pub fn send(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["--app-name=wkill", "--expire-time=3000", summary, body])
        .output();
}
