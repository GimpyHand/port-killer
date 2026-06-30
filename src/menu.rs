use crate::tui::{is_interactive_terminal, run_interactive};

#[cfg(feature = "gui")]
pub fn run_dropdown() -> Result<(), String> {
    crate::gui::run_dropdown()
}

#[cfg(not(feature = "gui"))]
pub fn run_dropdown() -> Result<(), String> {
    if is_interactive_terminal() {
        run_interactive()
    } else {
        Err("built without GUI; use port-killer menu --tui".to_string())
    }
}

#[cfg(feature = "gui")]
pub fn run_window() -> Result<(), String> {
    crate::gui::run_window()
}

#[cfg(not(feature = "gui"))]
pub fn run_window() -> Result<(), String> {
    run_dropdown()
}

/// Terminal picker, compact GTK dropdown, or full window.
pub fn run_menu(tui: bool, window: bool) -> Result<(), String> {
    if tui {
        return run_interactive();
    }

    if window {
        return run_window();
    }

    if display_available() {
        return run_dropdown();
    }

    if is_interactive_terminal() {
        return run_interactive();
    }

    Err("no display available — use port-killer menu --tui from a terminal".to_string())
}

pub fn display_available() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}
