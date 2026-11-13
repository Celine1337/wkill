import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import {Extension} from 'resource:///org/gnome/shell/misc/extensionUtils.js';

const DBUS_PATH = '/org/gnome/Shell/Extensions/Wkill';

const IFACE = `
<node>
  <interface name="org.gnome.Shell.Extensions.Wkill">
    <method name="List">
      <arg type="s" direction="out" name="windows"/>
    </method>
    <method name="Kill">
      <arg type="t" direction="in" name="id"/>
      <arg type="s" direction="in" name="mode"/>
      <arg type="s" direction="out" name="result"/>
    </method>
  </interface>
</node>`;

function esc(s) {
    return String(s ?? '')
        .replace(/\\/g, '\\\\')
        .replace(/\|/g, '\\|')
        .replace(/\n/g, '\\n')
        .replace(/\r/g, '\\r');
}

function selfPid() {
    const [ok, bytes] = GLib.file_get_contents('/proc/self/stat');
    if (!ok)
        return -1;
    return parseInt(new TextDecoder().decode(bytes).split(' ')[0], 10);
}

export default class WkillExtension extends Extension {
    enable() {
        this._impl = {
            List: () => {
                const lines = [];
                for (const actor of global.get_window_actors()) {
                    const w = actor.get_meta_window();
                    if (!w)
                        continue;
                    const t = w.get_window_type();
                    if (t !== Meta.WindowType.NORMAL &&
                        t !== Meta.WindowType.DIALOG &&
                        t !== Meta.WindowType.MODAL_DIALOG)
                        continue;
                    const client = w.get_client_type() === Meta.WindowClientType.X11 ? 'x11' : 'wayland';
                    lines.push([
                        w.get_id(),
                        w.get_pid(),
                        w.has_focus() ? 1 : 0,
                        client,
                        esc(w.get_wm_class()),
                        esc(w.get_title()),
                    ].join('|'));
                }
                return lines.join('\n');
            },
            Kill: (id, mode) => {
                let target = null;
                for (const actor of global.get_window_actors()) {
                    const w = actor.get_meta_window();
                    if (w && w.get_id() === id) {
                        target = w;
                        break;
                    }
                }
                if (!target)
                    throw new Error(`window ${id} not found`);
                if (mode === 'close') {
                    target.delete(global.get_current_time());
                    return 'ok';
                }
                if (mode !== 'term' && mode !== 'kill')
                    throw new Error(`unknown mode: ${mode}`);
                const pid = target.get_pid();
                if (pid <= 1 || pid === selfPid())
                    throw new Error('refusing to kill a system process');
                const sig = mode === 'term' ? 'TERM' : 'KILL';
                GLib.spawn_command_line_async(`kill -${sig} ${pid}`);
                return 'ok';
            },
        };
        this._dbus = Gio.DBusExportedObject.wrapJSObject(IFACE, this._impl);
        this._dbus.export(Gio.DBus.session, DBUS_PATH);
    }

    disable() {
        if (this._dbus) {
            this._dbus.unexport();
            this._dbus = null;
        }
        this._impl = null;
    }
}