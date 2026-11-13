<p align="center">
  <img src="assets/banner.svg" width="830" alt="wkill banner">
</p>
<p align="center">
  <a href="https://github.com/Celine1337/wkill/releases"><img src="assets/nav-releases.svg" alt="Releases"></a>
  <a href="https://gjs.guide/extensions/"><img src="assets/nav-spec.svg" alt="GJS guide"></a>
  <a href="LICENSE"><img src="assets/nav-mit.svg" alt="MIT"></a>
</p>

<img src="assets/section-about.svg" width="830" alt="About">

**wkill** is the **xkill that works on Wayland**. `xkill` is dead on modern Ubuntu — it speaks X11, and GNOME on Wayland deliberately gives clients no access to other windows. wkill brings the workflow back: press your hotkey and the **focused window dies**, or run `wkill pick` and choose a window from a list — the Wayland-friendly replacement for xkill's crosshair.

It is a zero-dependency **Rust CLI** plus a tiny **GNOME Shell bridge extension** (~100 lines, no UI, no settings) that exposes exactly two D-Bus methods: list windows, and close/kill one. The extension is the only legitimate way to get window identity on GNOME Wayland — the built-in `org.gnome.Shell.Introspect` API is locked down to portals. The hotkey itself is a **native GNOME custom keybinding**, so there is no daemon, no autostart, no background process: GNOME's own media-keys handler launches wkill on demand.

Nice touch: if you already bound a key to the broken `xkill`, `wkill install` **reuses that exact slot and keeps your combination** — muscle memory intact.

<img src="assets/section-features.svg" width="830" alt="Features">

- **One command to set up** — `wkill install` installs the bridge, copies the binary to `~/.local/bin`, and registers the hotkey.
- **Keeps your xkill hotkey** — an existing `xkill` keybinding is taken over in place, binding preserved.
- **Graceful first, brutal if needed** — close request → `SIGTERM` → `SIGKILL`, with liveness checks between steps; `--force` skips straight to `SIGKILL`.
- **Interactive picker** — `wkill pick [filter]` lists windows with PIDs and focus marker; type a number, done.
- **Safety guards** — refuses to kill `gnome-shell`/`mutter`/PID ≤ 1 on both the CLI and the extension side (the shell checks its own PID).
- **Desktop notifications** — every kill (or failure) is reported via `notify-send`.
- **Change the key anytime** — `wkill hotkey '<Super><Shift>q'`, no restart needed.

<img src="assets/section-stack.svg" width="830" alt="Tech stack">

<img src="assets/section-install.svg" width="830" alt="Install">

Requirements: GNOME 45–50 on Wayland (Ubuntu 24.04+ / Fedora 39+), Rust toolchain, and the standard `gdbus` / `gsettings` / `gnome-extensions` utilities.

```bash
git clone https://github.com/Celine1337/wkill.git
cd wkill
./build.sh
./target/release/wkill install
```

**One-time step on Wayland:** log out and back in once — GNOME only loads extensions at session start. After that, everything is instant. `wkill` (no arguments) tells you exactly what is active and what is waiting.

<img src="assets/section-usage.svg" width="830" alt="Usage">

```bash
wkill                  # status: session, extension, hotkey, bridge
wkill kill             # kill the focused window (what the hotkey runs)
wkill kill --force     # SIGKILL immediately
wkill pick             # choose a window from a numbered list
wkill pick firefox     # ...filtered by title or WM class
wkill list             # machine-readable-ish window table
wkill hotkey '<Super><Shift>q'
wkill hotkey off
wkill uninstall
```

Picker session:

```text
  #  pid     focus  title
  1  4242    *      Mozilla Firefox
  2  5150           org.gnome.Nautilus
  3  6001           wkill-test
kill which? [1-3] (q to quit): 3
closed 'wkill-test' (pid 6001)
```

How it works under the hood:

| Piece | Role |
| --- | --- |
| `extension/extension.js` | GNOME Shell extension: D-Bus `List` (id, pid, focus, client, WM class, title) and `Kill` (close / term / kill) |
| Rust CLI | talks to the bridge via `gdbus`, manages the keybinding via `gsettings`, escalates signals, notifies |
| GNOME media-keys | fires `wkill kill` on your hotkey — no daemon of ours runs in the background |

Run the checks (unit tests, clippy `-D warnings`, fmt, extension syntax, and a live kill of a sacrificial zenity window when the bridge is active):

```bash
./test.sh
```

Known boundaries, stated plainly:

- **GNOME only.** KDE/KWin and wlroots compositors have their own protocols and are out of scope.
- **One logout after install** is a Wayland fact of life, not a wkill limitation; the installer queues the extension in `enabled-extensions` so it activates on next login.
- Like xkill, wkill kills the **process**: a multi-window app loses all its windows. Graceful close is attempted first to let the app save state.
- The bridge exposes window titles to local D-Bus callers — same trust boundary as the shell itself; nothing leaves your machine.

<img src="assets/section-license.svg" width="830" alt="License">

MIT © 2026 Celine1337 — see [LICENSE](LICENSE).