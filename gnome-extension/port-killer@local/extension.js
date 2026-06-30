import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const PortKillerIndicator = GObject.registerClass(
class PortKillerIndicator extends PanelMenu.Button {
    _init(extensionDir, binary) {
        // 1.0 = align dropdown to right edge (status area icons sit on the right)
        super._init(1.0, 'Port Killer', false);
        this._binary = binary;
        this._targets = [];
        this._lastError = null;

        this._box = new St.BoxLayout({
            style_class: 'panel-status-menu-box',
            x_align: Clutter.ActorAlign.CENTER,
        });

        const iconPath = GLib.build_filenamev([extensionDir, 'icons', 'server-symbolic.svg']);
        const iconFile = Gio.File.new_for_path(iconPath);
        this._icon = new St.Icon({
            gicon: new Gio.FileIcon({ file: iconFile }),
            icon_size: 16,
            style_class: 'system-status-icon',
            y_align: Clutter.ActorAlign.CENTER,
        });

        this._countLabel = new St.Label({
            text: '…',
            y_align: Clutter.ActorAlign.CENTER,
            style_class: 'port-killer-count',
        });

        this._box.add_child(this._icon);
        this._box.add_child(this._countLabel);
        this.add_child(this._box);

        // Seed menu so the first click always has something to show.
        this.menu.addMenuItem(new PopupMenu.PopupMenuItem('Loading servers…', {
            reactive: false,
            can_focus: false,
        }));

        this._timeoutId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 5, () => {
            this._refresh();
            return GLib.SOURCE_CONTINUE;
        });
        this._refresh();
    }

    _buildPath() {
        const parts = (GLib.getenv('PATH') || '/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin').split(':');
        for (const dir of ['/usr/sbin', '/usr/bin', '/sbin', '/bin']) {
            if (!parts.includes(dir))
                parts.push(dir);
        }
        const localBin = GLib.build_filenamev([GLib.get_home_dir(), '.local', 'bin']);
        if (!parts.includes(localBin))
            parts.unshift(localBin);
        return parts.join(':');
    }

    _spawnArgs(args) {
        this._lastError = null;
        try {
            const launcher = new Gio.SubprocessLauncher();
            launcher.set_flags(
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE,
            );
            launcher.setenv('PATH', this._buildPath(), true);
            launcher.setenv('HOME', GLib.get_home_dir(), true);
            const proc = launcher.spawnv([this._binary, ...args]);
            const [ok, stdout, stderr] = proc.communicate_utf8(null, null);
            if (!ok) {
                this._lastError = stderr?.trim() || 'subprocess failed';
                return null;
            }
            return stdout?.trim() || null;
        } catch (e) {
            this._lastError = String(e);
            return null;
        }
    }

    _fetchStatus() {
        const raw = this._spawnArgs(['targets-json']);
        if (!raw)
            return { count: 0, targets: [] };

        try {
            const data = JSON.parse(raw);
            return {
                count: data.count ?? 0,
                targets: data.targets ?? [],
            };
        } catch (e) {
            this._lastError = String(e);
            return { count: 0, targets: [] };
        }
    }

    _buildMenu() {
        this.menu.removeAll();

        if (this._lastError) {
            this.menu.addMenuItem(new PopupMenu.PopupMenuItem(
                `port-killer error: ${this._lastError}`,
                { reactive: false, can_focus: false },
            ));
            return;
        }

        if (this._targets.length === 0) {
            this.menu.addMenuItem(new PopupMenu.PopupMenuItem(
                'No dev servers running',
                { reactive: false, can_focus: false },
            ));
            return;
        }

        for (const target of this._targets) {
            const label = target.label ?? ':?';
            const detail = target.detail ?? '';
            const index = target.index ?? 0;
            const text = detail ? `${label}  ·  ${detail}` : label;

            const item = new PopupMenu.PopupMenuItem(text, { reactive: true });
            item.connect('activate', () => {
                this._spawnArgs(['kill-group', String(index)]);
                GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
                    this._refresh();
                    return GLib.SOURCE_REMOVE;
                });
            });
            this.menu.addMenuItem(item);
        }
    }

    _refresh() {
        const status = this._fetchStatus();
        this._targets = status.targets;
        this._countLabel.text = String(status.count);
        this._box.tooltip_text = status.count === 0
            ? 'No dev servers — click for menu'
            : `${status.count} dev server(s) — click to kill`;
        this._icon.opacity = status.count === 0 ? 140 : 255;
        this._buildMenu();
    }

    destroy() {
        if (this._timeoutId) {
            GLib.source_remove(this._timeoutId);
            this._timeoutId = null;
        }
        super.destroy();
    }
});

export default class PortKillerExtension extends Extension {
    enable() {
        const binary = this.metadata['port-killer-binary'] || 'port-killer';
        this._indicator = new PortKillerIndicator(this.dir.get_path(), binary);
        Main.panel.addToStatusArea(this.uuid, this._indicator);
    }

    disable() {
        this._indicator?.destroy();
        this._indicator = null;
    }
}
