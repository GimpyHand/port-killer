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
        super._init(0.0, 'Port Killer', false);
        this._binary = binary;
        this._targets = [];

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
            text: '0',
            y_align: Clutter.ActorAlign.CENTER,
            style_class: 'port-killer-count',
        });

        this._box.add_child(this._icon);
        this._box.add_child(this._countLabel);
        this.add_child(this._box);

        this.menu.connect('open', () => {
            this._buildMenu();
        });

        this._timeoutId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 5, () => {
            this._refresh();
            return GLib.SOURCE_CONTINUE;
        });
        this._refresh();
    }

    _fetchTargets() {
        try {
            const [, stdout] = GLib.spawn_command_line_sync(`${this._binary} targets-json`);
            if (!stdout || stdout.length === 0)
                return [];
            const data = JSON.parse(new TextDecoder().decode(stdout));
            return data.targets ?? [];
        } catch (_e) {
            return [];
        }
    }

    _buildMenu() {
        this.menu.removeAll();
        this._targets = this._fetchTargets();

        if (this._targets.length === 0) {
            const item = new PopupMenu.PopupMenuItem('No dev servers running', {
                reactive: false,
                can_focus: false,
            });
            this.menu.addMenuItem(item);
            return;
        }

        for (const target of this._targets) {
            const label = target.label ?? ':?';
            const detail = target.detail ?? '';
            const index = target.index ?? 0;

            const item = new PopupMenu.PopupMenuItem(label, { reactive: true });
            if (detail) {
                item.label.clutter_text.set_text(`${label}\n${detail}`);
            }
            item.connect('activate', () => {
                GLib.spawn_command_line_async(`${this._binary} kill-group ${index}`);
            });
            this.menu.addMenuItem(item);
        }
    }

    _refresh() {
        try {
            const [, stdout] = GLib.spawn_command_line_sync(`${this._binary} list`);
            if (!stdout || stdout.length === 0)
                return;
            const data = JSON.parse(new TextDecoder().decode(stdout));
            const count = data.text ?? '0';
            this._countLabel.text = count;
            this._box.tooltip_text = data.tooltip ?? 'Dev servers — click for menu';
            this._icon.opacity = count === '0' ? 140 : 255;
        } catch (_e) {
            this._countLabel.text = '?';
            this._box.tooltip_text = 'port-killer error';
        }
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
