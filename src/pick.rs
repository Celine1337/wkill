use crate::dbus::{self, Window};
use crate::kill;
use std::io::{self, BufRead, Write};

pub fn run(filter: Option<&str>, force: bool) -> Result<(), String> {
    let windows = dbus::list_windows()?;
    let shown: Vec<&Window> = windows
        .iter()
        .filter(|w| match filter {
            Some(f) => {
                let f = f.to_lowercase();
                w.title.to_lowercase().contains(&f) || w.wm_class.to_lowercase().contains(&f)
            }
            None => true,
        })
        .collect();
    if shown.is_empty() {
        return Err("no matching windows".into());
    }
    println!("  #  pid     focus  title");
    for (i, w) in shown.iter().enumerate() {
        let mark = if w.focused { "*" } else { " " };
        let title = if w.title.is_empty() {
            &w.wm_class
        } else {
            &w.title
        };
        println!("{:>3}  {:<7} {}      {}", i + 1, w.pid, mark, title);
    }
    print!("kill which? [1-{}] (q to quit): ", shown.len());
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let line = line.trim();
    if line.is_empty() || line.eq_ignore_ascii_case("q") {
        return Ok(());
    }
    let n: usize = line.parse().map_err(|_| format!("not a number: {line}"))?;
    if n == 0 || n > shown.len() {
        return Err(format!("out of range: {n}"));
    }
    let w = shown[n - 1];
    let outcome = kill::kill_window(w, force)?;
    println!("{outcome}");
    crate::notify::send("wkill", &outcome);
    Ok(())
}
