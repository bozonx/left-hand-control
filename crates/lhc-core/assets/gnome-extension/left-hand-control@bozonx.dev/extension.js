// A fixed D-Bus API; no evaluation of caller-supplied JavaScript.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Keyboard from 'resource:///org/gnome/shell/ui/status/keyboard.js';

const IFACE = 'dev.bozonx.LeftHandControl.Gnome';
const PATH = '/dev/bozonx/LeftHandControl/Gnome';
const XML = `<node><interface name="${IFACE}">
<method name="Snapshot"><arg type="b" direction="in"/><arg type="s" direction="out"/></method>
<method name="Windows"><arg type="s" direction="out"/></method>
<method name="Activate"><arg type="s" direction="in"/></method>
<method name="Action"><arg type="s" direction="in"/></method>
<signal name="Changed"/>
</interface></node>`;

export default class Bridge extends Extension {
    enable() {
        this._epoch = GLib.uuid_string_random();
        this._externalId = null;
        this._sources = Keyboard.getInputSourceManager();
        this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._object.export(Gio.DBus.session, PATH);
        this._focusId = global.display.connect('notify::focus-window', () => this._watchWindow());
        this._sourceId = this._sources.connect('current-source-changed', () => this._changed());
        this._listId = this._sources.connect('sources-changed', () => this._changed());
        this._windowSignals = [];
        this._watchWindow();
    }

    disable() {
        this._changed();
        this._disconnectWindow();
        global.display.disconnect(this._focusId);
        this._sources.disconnect(this._sourceId);
        this._sources.disconnect(this._listId);
        this._object.unexport();
        this._object = null;
        this._sources = null;
    }

    _disconnectWindow() {
        for (const [window, id] of this._windowSignals)
            window.disconnect(id);
        this._windowSignals = [];
    }

    _watchWindow() {
        this._disconnectWindow();
        const window = global.display.focus_window;
        if (window) {
            if (window.get_wm_class() !== 'dev.bozonx.left-hand-control.popup')
                this._externalId = window.get_stable_sequence();
            for (const signal of ['notify::title', 'notify::fullscreen', 'position-changed', 'size-changed'])
                this._windowSignals.push([window, window.connect(signal, () => this._changed())]);
        }
        this._changed();
    }

    _changed() {
        this._object?.emit_signal('Changed', null);
    }

    // The bus authenticates the sender; check its Unix uid before exposing data
    // or manipulating the session. Never block the Shell on another D-Bus peer.
    _authorized(invocation, operation) {
        Gio.DBus.session.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetConnectionUnixUser',
            new GLib.Variant('(s)', [invocation.get_sender()]),
            new GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, 1000, null,
            (connection, result) => {
                try {
                    const [uid] = connection.call_finish(result).deepUnpack();
                    if (uid !== new Gio.Credentials().get_unix_user() || !this._object)
                        throw new Error('Session user or extension is unavailable');
                    if (Main.sessionMode.isLocked || Main.sessionMode.isGreeter)
                        throw new Error('Session is locked');
                    invocation.return_value(operation());
                } catch (error) {
                    invocation.return_dbus_error(error.dbusName ?? `${IFACE}.Unavailable`, error.message);
                }
            });
    }

    _window(window, titles) {
        if (!window)
            return null;
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        const rect = window.get_frame_rect();
        const monitor = Main.layoutManager.monitors[window.get_monitor()];
        return {
            windowId: `${this._epoch}:${window.get_stable_sequence()}`,
            appId: window.get_wm_class() ?? app?.get_id() ?? '',
            pid: window.get_pid(),
            title: titles ? window.get_title() ?? '' : '',
            fullscreen: window.is_fullscreen() || Boolean(window.get_frame_type() === Meta.FrameType.NONE && monitor &&
                rect.x <= monitor.x && rect.y <= monitor.y &&
                rect.x + rect.width >= monitor.x + monitor.width &&
                rect.y + rect.height >= monitor.y + monitor.height),
        };
    }

    SnapshotAsync([titles], invocation) {
        this._authorized(invocation, () => {
            const layouts = Object.values(this._sources.inputSources).map(source => {
                const [short, variant = ''] = source.xkbId.split('+');
                return {short, variant, index: source.index,
                    display: source.shortName, long: source.displayName};
            });
            const external = global.get_window_actors().map(actor => actor.meta_window)
                .find(window => window.get_stable_sequence() === this._externalId);
            return new GLib.Variant('(s)', [JSON.stringify({version: 1,
                window: this._window(Main.modalCount > 0 || Main.overview.visible
                    ? null : global.display.focus_window, titles),
                externalWindow: this._window(external, titles),
                layouts, current: this._sources.currentSource?.index ?? null,
                workspace: global.workspace_manager.get_active_workspace_index()})]);
        });
    }

    WindowsAsync(_params, invocation) {
        this._authorized(invocation, () => new GLib.Variant('(s)', [JSON.stringify(
            global.get_window_actors().map(actor => actor.meta_window)
                .filter(window => !window.is_skip_taskbar())
                .map(window => this._window(window, true)))]));
    }

    ActivateAsync([title], invocation) {
        Gio.DBus.session.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetConnectionUnixProcessID',
            new GLib.Variant('(s)', [invocation.get_sender()]),
            new GLib.VariantType('(u)'), Gio.DBusCallFlags.NONE, 1000, null,
            (connection, result) => {
                try {
                    const [pid] = connection.call_finish(result).deepUnpack();
                    this._authorized(invocation, () => {
                        const window = global.get_window_actors().map(actor => actor.meta_window)
                            .find(w => w.get_pid() === pid && w.get_title() === title);
                        if (!window) {
                            const error = new Error('Caller window is not mapped');
                            error.dbusName = `${IFACE}.NotMapped`;
                            throw error;
                        }
                        Main.activateWindow(window);
                        return null;
                    });
                } catch (error) {
                    invocation.return_dbus_error(`${IFACE}.Unavailable`, error.message);
                }
            });
    }

    ActionAsync([action], invocation) {
        this._authorized(invocation, () => {
            this._action(action);
            return null;
        });
    }

    _action(action) {
        const match = /^(switchDesktop|windowToDesktop|switchLayout)([1-9][0-9]*)$/.exec(action);
        const window = global.display.focus_window;
        if (match) {
            const index = Number(match[2]) - 1;
            if (match[1] === 'switchLayout') {
                const source = this._sources.inputSources[index];
                if (!source || index >= 10)
                    throw new Error('Input source is unavailable');
                source.activate(true);
                return;
            }
            const workspace = global.workspace_manager.get_workspace_by_index(index);
            if (!workspace || index >= 20)
                throw new Error('Workspace is unavailable');
            if (match[1] === 'switchDesktop')
                workspace.activate(global.get_current_time());
            else if (window)
                window.change_workspace(workspace);
            else
                throw new Error('No focused window');
            return;
        }
        switch (action) {
        case 'overview': Main.overview.toggle(); return;
        case 'appLauncher': Main.overview.showApps(); return;
        }
        if (!window)
            throw new Error('No focused window');
        switch (action) {
        case 'windowClose': window.delete(global.get_current_time()); return;
        case 'windowMinimize': window.minimize(); return;
        case 'windowMaximize':
            if (window.is_maximized())
                window.unmaximize();
            else
                window.maximize();
            return;
        case 'windowMaximizeVertical': {
            // Partial maximization is public again only in some Mutter
            // releases; without it the action reports itself unsupported.
            const vertical = Meta.MaximizeFlags.VERTICAL;
            const api = ['get_maximize_flags', 'set_maximize_flags', 'set_unmaximize_flags'];
            if (api.some(method => typeof window[method] !== 'function'))
                throw new Error('Unsupported action');
            if (window.get_maximize_flags() & vertical)
                window.set_unmaximize_flags(vertical);
            else
                window.set_maximize_flags(vertical);
            return;
        }
        case 'windowKeepAbove':
            if (window.is_above()) window.unmake_above(); else window.make_above();
            return;
        default: throw new Error('Unsupported action');
        }
    }
}
