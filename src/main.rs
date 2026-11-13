use std::process::ExitCode;
use wkill::{dbus, hotkey, install, kill, notify, pick};

const HELP: &str = "\
wkill — xkill for GNOME Wayland

USAGE:
    wkill                  status: extension, hotkey, bridge
    wkill kill [--force]   kill the focused window (this is what the hotkey runs)
    wkill pick [FILTER]    choose a window from a list and kill it
    wkill list             print open windows
    wkill hotkey BINDING   change the hotkey, e.g. '<Super><Shift>q'
    wkill hotkey off       remove the hotkey
    wkill install          install bridge extension + binary + hotkey
    wkill uninstall        remove hotkey, extension and stop

KILL ESCALATION:
    close request -> SIGTERM -> SIGKILL (use --force to skip straight to SIGKILL)

EXAMPLES:
    wkill install
    wkill hotkey '<Shift><Control>x'
    wkill pick firefox
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None => status(),
        Some("kill") => cmd_kill(args.iter().any(|a| a == "--force")),
        Some("pick") => pick::run(
            args.get(1).map(String::as_str).filter(|a| *a != "--force"),
            args.iter().any(|a| a == "--force"),
        ),
        Some("list") => cmd_list(),
        Some("hotkey") => cmd_hotkey(args.get(1).map(String::as_str)),
        Some("install") => cmd_install(),
        Some("uninstall") => cmd_uninstall(),
        Some("-h") | Some("--help") | Some("help") => {
            print!("{HELP}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command: {other}\n\n{HELP}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wkill: {e}");
            ExitCode::FAILURE
        }
    }
}

fn command_path() -> Result<String, String> {
    Ok(format!("{} kill", install::installed_bin()?.display()))
}

fn status() -> Result<(), String> {
    println!(
        "wkill {} — xkill for GNOME Wayland",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "session:    {} ({})",
        install::session_type(),
        install::gnome_version()
    );
    let bridge = dbus::bridge_ok();
    let mut state = install::extension_state();
    if bridge.is_ok() && (state == "ERROR" || state == "UNKNOWN") {
        state += " — stale shell cache, bridge is live; re-login clears this";
    }
    println!("extension:  {} ({})", state, install::UUID);
    let cmd = command_path()?;
    match hotkey::find_wkill_slot(&cmd)? {
        Some(s) => println!("hotkey:     {} → {}", s.binding, s.command),
        None => println!("hotkey:     not registered (run `wkill install`)"),
    }
    match bridge {
        Ok(n) => println!("bridge:     ok ({n} windows visible)"),
        Err(e) => println!("bridge:     {e}"),
    }
    Ok(())
}

fn cmd_kill(force: bool) -> Result<(), String> {
    let w = kill::focused()?;
    match kill::kill_window(&w, force) {
        Ok(outcome) => {
            notify::send("wkill", &outcome);
            Ok(())
        }
        Err(e) => {
            notify::send("wkill — error", &e);
            Err(e)
        }
    }
}

fn cmd_list() -> Result<(), String> {
    let ws = dbus::list_windows()?;
    if ws.is_empty() {
        println!("no windows");
        return Ok(());
    }
    println!(
        "{:<12} {:<8} {:<6} {:<8} {:<28} title",
        "id", "pid", "focus", "client", "wm_class"
    );
    for w in ws {
        println!(
            "{:<12} {:<8} {:<6} {:<8} {:<28} {}",
            w.id,
            w.pid,
            if w.focused { "*" } else { "" },
            w.client,
            w.wm_class,
            w.title
        );
    }
    Ok(())
}

fn cmd_hotkey(arg: Option<&str>) -> Result<(), String> {
    let cmd = command_path()?;
    match arg {
        None => match hotkey::find_wkill_slot(&cmd)? {
            Some(s) => {
                println!("{}", s.binding);
                Ok(())
            }
            None => Err("no wkill hotkey registered; run `wkill install` first".into()),
        },
        Some("off") => {
            if hotkey::unregister(&cmd)? {
                println!("hotkey removed");
            } else {
                println!("no wkill hotkey was registered");
            }
            Ok(())
        }
        Some(binding) => {
            if binding.is_empty() || binding.contains(' ') {
                return Err("binding must look like '<Ctrl><Alt>K' (no spaces)".into());
            }
            hotkey::set_binding(&cmd, binding)?;
            println!("hotkey set to {binding}");
            Ok(())
        }
    }
}

fn cmd_install() -> Result<(), String> {
    if install::session_type() != "wayland" {
        eprintln!("note: session is not Wayland; xkill itself may work here, installing anyway");
    }
    install::install_extension()?;
    let bin = install::install_binary()?;
    let cmd = format!("{} kill", bin.display());
    let binding = hotkey::register(&cmd, None)?;
    println!("extension installed: {}", install::UUID);
    println!("binary installed:    {}", bin.display());
    println!("hotkey:              {binding} → {cmd}");
    match dbus::bridge_ok() {
        Ok(_) => println!("bridge is live — press your hotkey on any window"),
        Err(_) => println!(
            "one-time step: log out and back in to activate the extension, then run `wkill`"
        ),
    }
    Ok(())
}

fn cmd_uninstall() -> Result<(), String> {
    let cmd = command_path()?;
    hotkey::unregister(&cmd)?;
    install::uninstall_extension()?;
    println!(
        "hotkey and extension removed; binary left at {}",
        install::installed_bin()?.display()
    );
    Ok(())
}
