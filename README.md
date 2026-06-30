# port-killer

List and kill dev TCP servers from the terminal or Waybar.

**Tested on Ubuntu 24.04 LTS and Ubuntu 26.04 LTS.**

## Quick install (Ubuntu 24 / 26)

```bash
git clone <repo-url> port-killer && cd port-killer
chmod +x install scripts/install-ubuntu-deps.sh

# Full stack: apt deps + build + Waybar module
./install --deps --waybar
```

Or step by step:

```bash
./scripts/install-ubuntu-deps.sh   # build-essential, iproute2, rofi, wofi, waybar
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # if no cargo
./install --waybar
```

Verify:

```bash
port-killer doctor
```

Binary installs to `~/.local/bin/port-killer`. Add to PATH if needed:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

### Ubuntu packages

| Package | Purpose |
|---------|---------|
| `build-essential` | Rust linker (`cc`) |
| `iproute2` | `ss` port scanning |
| `libgtk-4-dev` `libadwaita-1-dev` | Built-in kill window |
| `waybar` | Taskbar module (optional for CLI-only) |
| `pkg-config` | Rust build helper |

One-liner: `sudo apt install build-essential iproute2 libgtk-4-dev libadwaita-1-dev waybar`

## CLI usage

```bash
port-killer                 # interactive picker (default in a terminal)
port-killer status          # plain table (for scripts)
port-killer list            # JSON only — for Waybar, not humans
port-killer menu                 # built-in GTK kill window
port-killer menu --tui           # terminal picker instead
port-killer kill 12345      # kill by PID
```

**Interactive controls:** ↑↓ move, **space** toggle selection, **enter** confirm, **esc** cancel.

`port-killer list` is machine output for Waybar. Use `port-killer` or `port-killer status` in the terminal.

## Ubuntu default desktop (GNOME)

Ubuntu 24 / 26 use **GNOME Shell** top bar — not Waybar (unless you installed Hyprland/Sway yourself).

```bash
port-killer setup gnome --install
# or
./install --gnome
```

Then **restart GNOME Shell**:
- **X11:** `Alt+F2` → `r` → Enter
- **Wayland:** log out and back in

Look for the **server icon** and count on the top-right. Click → built-in kill window (GTK, matches Ubuntu theme).

Check: `port-killer setup gnome --check`

Auto-detect: `port-killer setup desktop --install`

## Waybar (Hyprland / Sway / i3 only)

After installing the CLI:

```bash
port-killer setup waybar --install
pkill waybar; waybar &
```

Or everything at once: `./install --waybar`

**Not showing on the bar?** Diagnose:

```bash
port-killer setup waybar --check
```

Common fix: Waybar does not inherit your shell PATH. The installer writes the **full path** to `port-killer` in config. If you use a custom Waybar config location:

```bash
port-killer setup waybar --install --config /path/to/your/config.jsonc
```

Restart Waybar after any config change.

## What it shows

- TCP listeners on ports **1024–65535**
- Excludes: 22, 53, 631, 5353, 9050, 11434
- Only **your** processes (UID check via `/proc`)

## Safety

- Won't kill other users' processes
- SIGTERM first; SIGKILL only if process survives

## Troubleshooting

```bash
port-killer doctor              # dependency + binary health check
port-killer setup waybar --check
```

**Ubuntu 24 / 26 fresh machine:** `./install --deps --waybar`

## Development

```bash
make build          # cargo build --release
make test           # cargo test
cargo run -- status
```
